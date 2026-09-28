//! "Edit locally": a tip's Markdown is written to an edit directory under the
//! app data folder, opened in the user's editor (VS Code by default), and
//! watched. Every save is pushed to GitHub as its own commit; when the local
//! copy is untouched and the tip changes elsewhere (Done, snooze, a remote
//! edit), the file is refreshed so the editor always shows the truth.

use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::state::AppState;
use crate::core::tip::Tip;

const EDIT_DIR: &str = "edit";
/// How often watched files are compared with what was last pushed.
const POLL: Duration = Duration::from_secs(2);

/// One file being edited. Lives in `AppState` so the watcher and the command
/// share it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSession {
    pub id: String,
    pub path: PathBuf,
    /// Content last written to or pushed from the file. A difference means
    /// the user saved something new.
    pub last: String,
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
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let existing = state.with(|i| i.edits.get(id).cloned());
    let keep_local = existing.is_some() && path.is_file();
    if !keep_local {
        std::fs::write(&path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        state.with(|i| {
            i.edits.insert(
                id.to_string(),
                EditSession {
                    id: id.to_string(),
                    path: path.clone(),
                    last: text,
                },
            )
        });
    }
    let command = state.settings().editor_command;
    launch(&command, &path)?;
    Ok(path)
}

/// Run the configured editor command with the file, falling back to the
/// system default application for `.md` files.
fn launch(command: &str, path: &Path) -> Result<(), String> {
    let command = command.trim();
    if !command.is_empty() {
        match spawn_editor(command, path) {
            Ok(()) => return Ok(()),
            Err(e) => log::warn!("editor {command:?} failed ({e}); falling back to the default app"),
        }
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| format!("cannot open {}: {e}", path.display()))
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
    let sessions: Vec<EditSession> = state.with(|i| i.edits.values().cloned().collect());
    for session in sessions {
        let Some(current) = state.board().get(&session.id).cloned() else {
            // Deleted from the board (locally or remotely): stop watching.
            state.with(|i| i.edits.remove(&session.id));
            continue;
        };
        let local = match std::fs::read_to_string(&session.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                state.with(|i| i.edits.remove(&session.id));
                continue;
            }
            Err(e) => {
                log::warn!("cannot read {}: {e}", session.path.display());
                continue;
            }
        };
        if local != session.last {
            push_local(app, &state, &session, &current, &local).await;
        } else {
            refresh_from_board(&state, &session, &current);
        }
    }
}

/// The user saved: parse and commit the file to GitHub.
async fn push_local(app: &AppHandle, state: &AppState, session: &EditSession, current: &Tip, local: &str) {
    let parsed = match Tip::parse(&current.path, current.sha.clone(), local) {
        Ok(tip) => tip,
        Err(e) => {
            // Mid-edit or broken frontmatter; try again after the next save.
            log::debug!("{} not pushed yet: {e}", session.path.display());
            return;
        }
    };
    let msg = format!("vindictive: edit \"{}\" locally", parsed.front.title);
    match state.save_tip(app, parsed, &msg).await {
        Ok(saved) => {
            // Canonical form back to disk so the next tick sees no difference.
            let canonical = saved.to_markdown().unwrap_or_else(|_| local.to_string());
            if canonical != local {
                if let Err(e) = std::fs::write(&session.path, &canonical) {
                    log::warn!("cannot rewrite {}: {e}", session.path.display());
                }
            }
            remember(state, &session.id, canonical);
            log::info!("pushed local edit of {}", session.id);
        }
        Err(e) => log::warn!("local edit of {} not pushed: {e}", session.id),
    }
}

/// The board changed while the file was untouched: refresh the file.
fn refresh_from_board(state: &AppState, session: &EditSession, current: &Tip) {
    let Ok(text) = current.to_markdown() else { return };
    if text == session.last {
        return;
    }
    match std::fs::write(&session.path, &text) {
        Ok(()) => remember(state, &session.id, text),
        Err(e) => log::warn!("cannot refresh {}: {e}", session.path.display()),
    }
}

fn remember(state: &AppState, id: &str, last: String) {
    state.with(|i| {
        if let Some(s) = i.edits.get_mut(id) {
            s.last = last;
        }
    });
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
}
