//! "Edit": open a tip's own file in the tips folder in the user's editor
//! (VS Code by default). There is no copy and no push step: saving the file
//! is the edit, and the folder scan puts it on the board within seconds.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::state::AppState;

/// Open the tip's file in the editor; returns its path.
pub fn open(state: &AppState, id: &str) -> Result<PathBuf, String> {
    let tip = state.tip(id).ok_or_else(|| format!("tip {id} not found"))?;
    let path = state
        .store()?
        .resolve(&tip.path)
        .map_err(|e| e.to_string())?;
    launch(&state.settings().editor_command, &path)?;
    Ok(path)
}

/// Well-known VS Code executables, for when `code` is not on the PATH the
/// app inherited (common when launched from a shortcut or another shell).
fn known_editor(command: &str) -> Option<PathBuf> {
    if !command.eq_ignore_ascii_case("code") {
        return None;
    }
    let mut candidates = Vec::new();
    candidates.extend(registry_code_exe());
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Programs/Microsoft VS Code/Code.exe"));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(pf) = std::env::var(var) {
            candidates.push(PathBuf::from(pf).join("Microsoft VS Code/Code.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// The `Code.exe` VS Code registered as an application, wherever it was
/// installed (profiles on other drives, system-wide installs).
#[cfg(windows)]
fn registry_code_exe() -> Vec<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const KEYS: [&str; 3] = [
        r"HKCU\Software\Classes\Applications\Code.exe\shell\open\command",
        r"HKLM\Software\Classes\Applications\Code.exe\shell\open\command",
        r"HKCR\Applications\Code.exe\shell\open\command",
    ];
    KEYS.iter()
        .filter_map(|key| {
            let out = Command::new("reg")
                .args(["query", key, "/ve"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok()?;
            parse_open_command(&String::from_utf8_lossy(&out.stdout))
        })
        .collect()
}

#[cfg(not(windows))]
fn registry_code_exe() -> Vec<PathBuf> {
    Vec::new()
}

/// First quoted path in a `reg query` dump of an `open\command` value,
/// e.g. `"D:\...\Code.exe" "%1"` → `D:\...\Code.exe`.
fn parse_open_command(dump: &str) -> Option<PathBuf> {
    let start = dump.find('"')? + 1;
    let end = start + dump[start..].find('"')?;
    let path = dump[start..end].trim();
    (!path.is_empty() && path.to_ascii_lowercase().ends_with(".exe")).then(|| PathBuf::from(path))
}

/// Run the configured editor command with the file, falling back to the
/// system default application for `.md` files.
fn launch(command: &str, path: &Path) -> Result<(), String> {
    let command = command.trim();
    if !command.is_empty() {
        match spawn_editor(command, path) {
            Ok(()) => return Ok(()),
            Err(e) => log::warn!("editor {command:?} failed ({e})"),
        }
        if let Some(exe) = known_editor(command) {
            match Command::new(&exe).arg(path).spawn() {
                Ok(_) => return Ok(()),
                Err(e) => log::warn!("{} failed ({e})", exe.display()),
            }
        }
        log::warn!("falling back to the default app for {}", path.display());
    }
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))
}

/// Start the configured editor on `path` without waiting for it to exit.
///
/// The command is never handed to `cmd` as a string: it is resolved to a file
/// first (a bare name is looked up on the PATH as `.exe`, `.cmd`, `.bat`), and
/// an `.exe` is started directly with the path as one argument. Only a batch
/// script needs `cmd`, and then any path `cmd` could reinterpret (`%VAR%`,
/// `&`, `|`, ...) is refused, so the caller falls back to `Code.exe` or the
/// default app.
fn spawn_editor(command: &str, path: &Path) -> Result<(), String> {
    let command = command.trim().trim_matches('"').trim();
    let program = resolve_command(command, std::env::var_os("PATH").as_deref())
        .ok_or_else(|| format!("{command:?} not found on the PATH"))?;
    if is_batch(&program) {
        spawn_batch(&program, path)
    } else {
        Command::new(&program)
            .arg(path)
            .spawn()
            .map(drop)
            .map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
fn spawn_batch(script: &Path, path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let line = batch_command_line(script, path)?;
    Command::new("cmd")
        .raw_arg(line)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn spawn_batch(script: &Path, _path: &Path) -> Result<(), String> {
    Err(format!("{} is a Windows batch file", script.display()))
}

/// Extensions tried, in order, for a command given without one.
const EXTENSIONS: [&str; 3] = ["exe", "cmd", "bat"];

/// The file `command` names: a path as given (or with an added extension),
/// or a bare name looked up in each `PATH` directory in turn. Directory
/// order wins over extension order, as in `cmd.exe`.
fn resolve_command(command: &str, path_var: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    if command.is_empty() {
        return None;
    }
    let as_path = Path::new(command);
    let has_dir = command.contains(['/', '\\']) || as_path.is_absolute();
    let dirs: Vec<PathBuf> = if has_dir {
        vec![PathBuf::new()]
    } else {
        path_var
            .map(|v| std::env::split_paths(v).collect())
            .unwrap_or_default()
    };
    let has_ext = as_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)) || e.eq_ignore_ascii_case("com")
        });
    dirs.iter().find_map(|dir| {
        let base = dir.join(command);
        if has_ext {
            return base.is_file().then_some(base);
        }
        EXTENSIONS
            .iter()
            .map(|ext| {
                let mut name = base.clone().into_os_string();
                name.push(".");
                name.push(ext);
                PathBuf::from(name)
            })
            .chain(has_dir.then(|| base.clone()))
            .find(|p| p.is_file())
    })
}

fn is_batch(program: &Path) -> bool {
    program
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// Characters `cmd` acts on even inside quotes (`%` expansion) or that could
/// end the quoting; a path containing one is never given to `cmd`.
const CMD_UNSAFE: [char; 9] = ['%', '"', '&', '^', '|', '<', '>', '\r', '\n'];

fn cmd_safe(text: &str) -> bool {
    !text.contains(CMD_UNSAFE)
}

/// `cmd` arguments running `script` on `path`: `/D` skips AutoRun, `/V:OFF`
/// keeps `!` literal, and `/S` strips exactly the outer pair of quotes.
fn batch_command_line(script: &Path, path: &Path) -> Result<String, String> {
    let (script, path) = (script.display().to_string(), path.display().to_string());
    if !cmd_safe(&script) || !cmd_safe(&path) {
        return Err(format!(
            "{path:?} has characters cmd.exe would interpret; not passing it to {script:?}"
        ));
    }
    Ok(format!("/D /V:OFF /S /C \"\"{script}\" \"{path}\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reg_query_output() {
        let dump = "\r\nHKEY_CURRENT_USER\\Software\\Classes\\Applications\\Code.exe\\shell\\open\\command\r\n    (Default)    REG_SZ    \"D:\\Users\\me\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe\" \"%1\"\r\n\r\n";
        assert_eq!(
            parse_open_command(dump),
            Some(PathBuf::from(
                r"D:\Users\me\AppData\Local\Programs\Microsoft VS Code\Code.exe"
            ))
        );
        assert_eq!(
            parse_open_command(
                "ERROR: The system was unable to find the specified registry key or value."
            ),
            None
        );
        assert_eq!(parse_open_command("\"not an exe\" \"%1\""), None);
    }

    #[test]
    fn cmd_never_sees_expandable_or_operator_characters() {
        let script = Path::new(r"C:\Tools\code.cmd");
        for bad in [
            r"C:\tips\x%COMSPEC%.md",
            r"C:\tips\a&calc.md",
            r"C:\tips\a^b.md",
            r"C:\tips\a|b.md",
            r"C:\tips\a<b>.md",
        ] {
            assert!(batch_command_line(script, Path::new(bad)).is_err(), "{bad}");
        }
        assert!(batch_command_line(Path::new(r"C:\100%\code.cmd"), Path::new("a.md")).is_err());
        assert_eq!(
            batch_command_line(script, Path::new(r"C:\My tips\Urgent! (v2).md")).unwrap(),
            r#"/D /V:OFF /S /C ""C:\Tools\code.cmd" "C:\My tips\Urgent! (v2).md"""#
        );
    }

    #[test]
    fn resolves_bare_names_on_the_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(first.path().join("code.cmd"), "").unwrap();
        std::fs::write(second.path().join("code.exe"), "").unwrap();
        std::fs::write(second.path().join("vim.bat"), "").unwrap();
        let joined = std::env::join_paths([first.path(), second.path()]).unwrap();
        let path_var = Some(joined.as_os_str());
        // Directory order wins over extension order, as in cmd.exe.
        assert_eq!(
            resolve_command("code", path_var),
            Some(first.path().join("code.cmd"))
        );
        assert_eq!(
            resolve_command("code.exe", path_var),
            Some(second.path().join("code.exe"))
        );
        assert_eq!(
            resolve_command("vim", path_var),
            Some(second.path().join("vim.bat"))
        );
        assert_eq!(resolve_command("nope", path_var), None);
        assert_eq!(resolve_command("", path_var), None);
        assert_eq!(resolve_command("code", None), None);
        let full = second.path().join("code");
        assert_eq!(
            resolve_command(full.to_str().unwrap(), None),
            Some(second.path().join("code.exe"))
        );
        assert!(is_batch(Path::new("x.CMD")) && is_batch(Path::new("x.bat")));
        assert!(!is_batch(Path::new("x.exe")));
    }
}
