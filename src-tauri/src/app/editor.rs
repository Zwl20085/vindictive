//! "Edit locally": a tip's Markdown is written to an edit directory under the
//! app data folder, opened in the user's editor (VS Code by default), and
//! watched. Saves are collected as *pending* and committed to GitHub in one
//! push pass every `push_interval_minutes` (0 = at once) or when the user
//! picks "Commit & push now". When the local copy is untouched and the tip
//! changes elsewhere (Done, snooze, a remote edit), the file is refreshed so
//! the editor always shows the truth. Sessions persist across restarts.

use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::state::{now, AppState};
use crate::core::tip::Tip;

const EDIT_DIR: &str = "edit";
/// How often watched files are compared with what was last pushed.
const POLL: Duration = Duration::from_secs(2);

/// One file being edited. Lives in `AppState` and in `edits.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditSession {
    pub id: String,
    pub path: PathBuf,
    /// Content last written to or pushed from the file.
    pub last: String,
    /// Latest saved content that has not been pushed yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<String>,
}

/// Directory holding local copies: `<app data>/edit/<repo path>`.
pub fn edit_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("no app data dir: {e}"))?;
    Ok(dir.join(EDIT_DIR))
}

/// Map a repository path to a file under `root`, refusing anything that
/// could escape it.
pub fn local_path(root: &Path, repo_path: &str) -> Result<PathBuf, String> {
    let rel = Path::new(repo_path);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("refusing to edit path {repo_path:?}"));
    }
    Ok(root.join(rel))
}

/// Write the tip to its local file (unless a session already holds newer
/// local edits), start watching it, and open it in the editor.
pub fn open(app: &AppHandle, state: &AppState, id: &str) -> Result<PathBuf, String> {
    let tip = state
        .board()
        .get(id)
        .cloned()
        .ok_or_else(|| format!("tip {id} not found"))?;
    let text = tip.to_markdown().map_err(|e| e.to_string())?;
    let path = local_path(&edit_dir(app)?, &tip.path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let existing = state.with(|i| i.edits.get(id).cloned());
    let keep_local = existing.is_some() && path.is_file();
    if !keep_local {
        std::fs::write(&path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        state.update_edits(|edits| {
            edits.insert(
                id.to_string(),
                EditSession {
                    id: id.to_string(),
                    path: path.clone(),
                    last: text,
                    pending: None,
                },
            );
        });
    }
    let command = state.settings().editor_command;
    launch(&command, &path)?;
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

/// Background watcher for every open session.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(POLL).await;
            tick(&app).await;
        }
    });
}

async fn tick(app: &AppHandle) {
    let state = AppState::from_app(app);
    collect_saves(&state);
    if push_due(&state) {
        push_pending(app, &state).await;
    }
}

/// Note every file whose content differs from what was last pushed, and
/// refresh untouched files whose tip changed on the board.
fn collect_saves(state: &AppState) {
    let sessions: Vec<EditSession> = state.with(|i| i.edits.values().cloned().collect());
    for session in sessions {
        let Some(current) = state.board().get(&session.id).cloned() else {
            // Deleted from the board (locally or remotely): stop watching.
            state.update_edits(|e| {
                e.remove(&session.id);
            });
            continue;
        };
        let local = match std::fs::read_to_string(&session.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                state.update_edits(|e| {
                    e.remove(&session.id);
                });
                continue;
            }
            Err(e) => {
                log::warn!("cannot read {}: {e}", session.path.display());
                continue;
            }
        };
        if local != session.last {
            if session.pending.as_deref() != Some(local.as_str()) {
                state.update_edits(|e| {
                    if let Some(s) = e.get_mut(&session.id) {
                        s.pending = Some(local.clone());
                    }
                });
            }
        } else if session.pending.is_some() {
            // Reverted by hand to the pushed version: nothing to push.
            state.update_edits(|e| {
                if let Some(s) = e.get_mut(&session.id) {
                    s.pending = None;
                }
            });
        } else {
            refresh_from_board(state, &session, &current);
        }
    }
}

/// Whether the scheduled push should run now.
fn push_due(state: &AppState) -> bool {
    let (interval, last_push, pending) = state.with(|i| {
        (
            i.settings.push_interval_minutes,
            i.last_push,
            i.edits.values().any(|s| s.pending.is_some()),
        )
    });
    if !pending {
        return false;
    }
    match (interval, last_push) {
        (0, _) | (_, None) => true,
        (minutes, Some(last)) => now() - last >= chrono::Duration::minutes(minutes as i64),
    }
}

/// Commit every pending edit to GitHub, one commit per file. Returns how
/// many were pushed. Files that do not parse stay pending.
pub async fn push_pending(app: &AppHandle, state: &AppState) -> usize {
    let sessions: Vec<EditSession> = state.with(|i| {
        i.edits
            .values()
            .filter(|s| s.pending.is_some())
            .cloned()
            .collect()
    });
    let mut pushed = 0;
    for session in sessions {
        let Some(local) = session.pending.clone() else { continue };
        let Some(current) = state.board().get(&session.id).cloned() else { continue };
        let parsed = match Tip::parse(&current.path, current.sha.clone(), &local) {
            Ok(tip) => tip,
            Err(e) => {
                log::warn!("{} not pushed (fix the file and save again): {e}", session.path.display());
                continue;
            }
        };
        let msg = format!("vindictive: edit \"{}\" locally", parsed.front.title);
        match state.save_tip(app, parsed, &msg).await {
            Ok(saved) => {
                // Canonical form back to disk so the next tick sees no difference.
                let canonical = saved.to_markdown().unwrap_or_else(|_| local.clone());
                if canonical != local {
                    if let Err(e) = std::fs::write(&session.path, &canonical) {
                        log::warn!("cannot rewrite {}: {e}", session.path.display());
                    }
                }
                state.update_edits(|e| {
                    if let Some(s) = e.get_mut(&session.id) {
                        s.last = canonical;
                        s.pending = None;
                    }
                });
                pushed += 1;
                log::info!("pushed local edit of {}", session.id);
            }
            Err(e) => log::warn!("local edit of {} not pushed: {e}", session.id),
        }
    }
    state.with(|i| i.last_push = Some(now()));
    if pushed > 0 {
        state.emit(app);
    }
    pushed
}

/// The board changed while the file was untouched: refresh the file.
fn refresh_from_board(state: &AppState, session: &EditSession, current: &Tip) {
    let Ok(text) = current.to_markdown() else { return };
    if text == session.last {
        return;
    }
    match std::fs::write(&session.path, &text) {
        Ok(()) => state.update_edits(|e| {
            if let Some(s) = e.get_mut(&session.id) {
                s.last = text;
            }
        }),
        Err(e) => log::warn!("cannot refresh {}: {e}", session.path.display()),
    }
}

/// When the next scheduled push is due, for display.
pub fn next_push_at(interval_minutes: u64, last_push: Option<NaiveDateTime>) -> Option<NaiveDateTime> {
    match (interval_minutes, last_push) {
        (0, _) | (_, None) => None,
        (m, Some(last)) => Some(last + chrono::Duration::minutes(m as i64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_paths_stay_inside_the_edit_dir() {
        let root = Path::new("C:/data/edit");
        assert_eq!(
            local_path(root, "tips/a.md").unwrap(),
            root.join("tips").join("a.md")
        );
        assert!(local_path(root, "../a.md").is_err());
        assert!(local_path(root, "/abs.md").is_err());
        assert!(local_path(root, "tips/../../x.md").is_err());
    }

    #[test]
    fn parses_reg_query_output() {
        let dump = "\r\nHKEY_CURRENT_USER\\Software\\Classes\\Applications\\Code.exe\\shell\\open\\command\r\n    (Default)    REG_SZ    \"D:\\Users\\me\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe\" \"%1\"\r\n\r\n";
        assert_eq!(
            parse_open_command(dump),
            Some(PathBuf::from(r"D:\Users\me\AppData\Local\Programs\Microsoft VS Code\Code.exe"))
        );
        assert_eq!(parse_open_command("ERROR: The system was unable to find the specified registry key or value."), None);
        assert_eq!(parse_open_command("\"not an exe\" \"%1\""), None);
    }

    #[test]
    fn next_push_maths() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 9, 28)
            .unwrap()
            .and_hms_opt(13, 0, 0)
            .unwrap();
        assert_eq!(next_push_at(0, Some(t)), None);
        assert_eq!(next_push_at(60, None), None);
        assert_eq!(
            next_push_at(60, Some(t)),
            Some(t + chrono::Duration::hours(1))
        );
    }

    #[test]
    fn session_roundtrips_json() {
        let s = EditSession {
            id: "a".into(),
            path: PathBuf::from("C:/x/a.md"),
            last: "---\ntitle: A\n---\n".into(),
            pending: Some("---\ntitle: B\n---\n".into()),
        };
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<EditSession>(&text).unwrap(), s);
    }
}
