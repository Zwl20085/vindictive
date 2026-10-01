//! "Edit": open a tip's own file in the tips folder in the user's editor
//! (VS Code by default). There is no copy and no push step: saving the file
//! is the edit, and the folder scan puts it on the board within seconds.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::state::AppState;

/// Open the tip's file in the editor; returns its path.
pub fn open(state: &AppState, id: &str) -> Result<PathBuf, String> {
    let tip = state
        .board()
        .get(id)
        .cloned()
        .ok_or_else(|| format!("tip {id} not found"))?;
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

#[cfg(windows)]
fn spawn_editor(command: &str, path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // `code` is code.cmd on Windows, so go through cmd.exe. The doubled outer
    // quotes are cmd's rule for a quoted command followed by a quoted argument.
    let status = Command::new("cmd")
        .raw_arg(format!("/C \"\"{command}\" \"{}\"\"", path.display()))
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("exit status {status}"))
    }
}

#[cfg(not(windows))]
fn spawn_editor(command: &str, path: &Path) -> Result<(), String> {
    Command::new(command)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
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
}
