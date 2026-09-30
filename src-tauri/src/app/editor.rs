//! "Edit locally": a tip's Markdown is written to an edit directory under the
//! app data folder, opened in the user's editor (VS Code by default), and
//! watched. Saves are collected as *pending* and committed to GitHub in one
//! push pass every `push_interval_minutes` (0 = at once) or when the user
//! picks "Commit & push now". When the local copy is untouched and the tip
//! changes elsewhere (Done, snooze, a remote edit), the file is refreshed so
//! the editor always shows the truth. Sessions persist across restarts.
//!
//! A push is based on the SHA the local copy started from, so a tip that
//! changed elsewhere meanwhile is a conflict, never a silent overwrite. The
//! local copy is kept, GitHub's version is written next to it as
//! `<name>.remote.md`, and nothing is pushed until the user saves again.

use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::notify;
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
    /// Blob SHA of the version `last` came from; pushes are based on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    /// The tip changed on GitHub under a pending edit. Held back until the
    /// user saves the file again (after merging `<name>.remote.md`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub conflict: bool,
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
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
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
        std::fs::write(&path, &text)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        state.update_edits(|edits| {
            edits.insert(
                id.to_string(),
                EditSession {
                    id: id.to_string(),
                    path: path.clone(),
                    last: text,
                    pending: None,
                    base_sha: tip.sha.clone(),
                    conflict: false,
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
    // A missing tip only means "deleted" once a sync has confirmed it; before
    // that the board may just be empty (fresh start, repo switch).
    let synced = state.with(|i| i.last_sync.is_some() && i.sync_error.is_none());
    for session in sessions {
        let Some(current) = state.board().get(&session.id).cloned() else {
            if session.pending.is_none() || synced {
                if session.pending.is_some() {
                    log::warn!(
                        "{} was deleted; its unpushed local copy stays at {}",
                        session.id,
                        session.path.display()
                    );
                }
                state.update_edits(|e| {
                    e.remove(&session.id);
                });
            }
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
                // A new save; after a conflict it means the user merged.
                state.update_edits(|e| {
                    if let Some(s) = e.get_mut(&session.id) {
                        s.pending = Some(local.clone());
                        s.conflict = false;
                    }
                });
            }
        } else if session.pending.is_some() {
            // Reverted by hand to the pushed version: nothing to push.
            state.update_edits(|e| {
                if let Some(s) = e.get_mut(&session.id) {
                    s.pending = None;
                    s.conflict = false;
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
            i.edits.values().any(|s| s.pending.is_some() && !s.conflict),
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
            .filter(|s| s.pending.is_some() && !s.conflict)
            .cloned()
            .collect()
    });
    let mut pushed = 0;
    for session in sessions {
        let Some(local) = session.pending.clone() else {
            continue;
        };
        let Some(current) = state.board().get(&session.id).cloned() else {
            continue;
        };
        let base = session.base_sha.clone().or_else(|| current.sha.clone());
        let parsed = match Tip::parse(&current.path, base.clone(), &local) {
            Ok(tip) => tip,
            Err(e) => {
                log::warn!(
                    "{} not pushed (fix the file and save again): {e}",
                    session.path.display()
                );
                continue;
            }
        };
        let msg = format!("vindictive: edit \"{}\" locally", parsed.front.title);
        match state.save_tip(app, parsed, &msg).await {
            Ok(saved) => {
                // Canonical form back to disk so the next tick sees no
                // difference, unless the user saved again during the push:
                // that newer save is picked up as pending on the next tick.
                let canonical = saved.to_markdown().unwrap_or_else(|_| local.clone());
                let untouched = std::fs::read_to_string(&session.path)
                    .map(|on_disk| on_disk == local)
                    .unwrap_or(false);
                if untouched && canonical != local {
                    if let Err(e) = std::fs::write(&session.path, &canonical) {
                        log::warn!("cannot rewrite {}: {e}", session.path.display());
                    }
                }
                state.update_edits(|e| {
                    if let Some(s) = e.get_mut(&session.id) {
                        s.last = canonical;
                        s.pending = None;
                        s.base_sha = saved.sha.clone();
                    }
                });
                pushed += 1;
                log::info!("pushed local edit of {}", session.id);
            }
            Err(e) => {
                // `save_tip` resynced; a new SHA means the tip moved on.
                let latest = state.board().get(&session.id).cloned();
                match latest.filter(|t| t.sha.is_some() && t.sha != base) {
                    Some(remote) => mark_conflict(app, state, &session, &remote),
                    None => log::warn!("local edit of {} not pushed: {e}", session.id),
                }
            }
        }
    }
    state.with(|i| i.last_push = Some(now()));
    if pushed > 0 {
        state.emit(app);
    }
    pushed
}

/// `tips/a.md` -> `tips/a.remote.md`, where GitHub's side of a conflict goes.
pub fn remote_copy_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{stem}.remote.md"))
}

/// Hold a pending edit back: GitHub has a newer version of the tip.
fn mark_conflict(app: &AppHandle, state: &AppState, session: &EditSession, remote: &Tip) {
    let copy = remote_copy_path(&session.path);
    match remote.to_markdown() {
        Ok(text) => {
            if let Err(e) = std::fs::write(&copy, text) {
                log::warn!("cannot write {}: {e}", copy.display());
            }
        }
        Err(e) => log::warn!("cannot render {}: {e}", remote.id),
    }
    state.update_edits(|e| {
        if let Some(s) = e.get_mut(&session.id) {
            s.conflict = true;
            s.base_sha = remote.sha.clone();
        }
    });
    log::warn!(
        "local edit of {} conflicts with GitHub; kept, remote copy at {}",
        session.id,
        copy.display()
    );
    let name = copy.file_name().unwrap_or_default().to_string_lossy();
    notify::show(
        app,
        &format!("Not pushed: {}", remote.display_title()),
        &format!(
            "It changed on GitHub while you edited it. Your copy is kept; \
             GitHub's version is in {name}. Merge and save to push."
        ),
    );
}

/// The board changed while the file was untouched: refresh the file.
fn refresh_from_board(state: &AppState, session: &EditSession, current: &Tip) {
    let Ok(text) = current.to_markdown() else {
        return;
    };
    if text == session.last {
        return;
    }
    match std::fs::write(&session.path, &text) {
        Ok(()) => state.update_edits(|e| {
            if let Some(s) = e.get_mut(&session.id) {
                s.last = text;
                s.base_sha = current.sha.clone();
            }
        }),
        Err(e) => log::warn!("cannot refresh {}: {e}", session.path.display()),
    }
}

/// When the next scheduled push is due, for display.
pub fn next_push_at(
    interval_minutes: u64,
    last_push: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
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
            base_sha: Some("abc".into()),
            conflict: true,
        };
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<EditSession>(&text).unwrap(), s);
        // Sessions saved by older versions still load.
        let old = r#"{"id":"a","path":"C:/x/a.md","last":"x"}"#;
        let loaded: EditSession = serde_json::from_str(old).unwrap();
        assert_eq!((loaded.base_sha, loaded.conflict), (None, false));
    }

    #[test]
    fn remote_copy_sits_next_to_the_file() {
        assert_eq!(
            remote_copy_path(Path::new("C:/data/edit/tips/a.md")),
            Path::new("C:/data/edit/tips/a.remote.md")
        );
    }

    fn state_in(dir: &Path) -> AppState {
        AppState::load(super::super::storage::Storage::new(dir.join("data")))
    }

    fn watch(dir: &Path, state: &AppState, id: &str, on_disk: &str) -> PathBuf {
        let path = dir.join(format!("{id}.md"));
        std::fs::write(&path, on_disk).unwrap();
        state.update_edits(|e| {
            e.insert(
                id.into(),
                EditSession {
                    id: id.into(),
                    path: path.clone(),
                    last: "---\ntitle: A\n---\n".into(),
                    pending: None,
                    base_sha: Some("1".into()),
                    conflict: false,
                },
            );
        });
        path
    }

    #[test]
    fn unpushed_edit_survives_an_empty_board_until_a_sync_confirms() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let tip = Tip::parse("tips/a.md", Some("1".into()), "---\ntitle: A\n---\n").unwrap();
        state.replace_board(super::super::board::Board::new(vec![tip]));
        watch(dir.path(), &state, "a", "---\ntitle: A edited\n---\n");
        collect_saves(&state);
        assert!(state.with(|i| i.edits["a"].pending.is_some()));
        // Board emptied before any sync (repo switch, unreadable cache): keep it.
        state.replace_board(Default::default());
        collect_saves(&state);
        assert!(state.with(|i| i.edits["a"].pending.is_some()));
        // A successful sync without the tip: now it really is gone.
        state.with(|i| i.last_sync = Some(now()));
        collect_saves(&state);
        assert!(state.with(|i| i.edits.is_empty()));
    }

    #[test]
    fn saving_again_clears_a_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_in(dir.path());
        let tip = Tip::parse("tips/a.md", Some("2".into()), "---\ntitle: A\n---\n").unwrap();
        state.replace_board(super::super::board::Board::new(vec![tip]));
        let path = watch(dir.path(), &state, "a", "---\ntitle: mine\n---\n");
        collect_saves(&state);
        state.update_edits(|e| e.get_mut("a").unwrap().conflict = true);
        collect_saves(&state);
        assert!(
            state.with(|i| i.edits["a"].conflict),
            "unchanged file stays held"
        );
        std::fs::write(&path, "---\ntitle: merged\n---\n").unwrap();
        collect_saves(&state);
        let s = state.with(|i| i.edits["a"].clone());
        assert!(!s.conflict);
        assert_eq!(s.pending.as_deref(), Some("---\ntitle: merged\n---\n"));
    }
}
