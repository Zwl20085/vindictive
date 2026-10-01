//! Shared application state and the sync engine.
//!
//! Locking discipline: the mutex is held only for short, non-async sections.
//! File reads and writes always happen on a cloned snapshot.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::{Local, NaiveDateTime, Timelike};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::board::Board;
use super::settings::Settings;
use super::storage::Storage;
use crate::core::nextup::{next_up, ordered};
use crate::core::tip::Tip;
use crate::sync::folder::{FolderError, FolderStore};
use crate::sync::weather::Weather;

pub const EVENT_BOARD_UPDATED: &str = "board-updated";
const IMAGE_CACHE_MAX: usize = 64;
/// Weather older than this is refetched.
pub const WEATHER_FRESH_MINUTES: i64 = 20;

#[derive(Debug, Default)]
pub struct Inner {
    pub settings: Settings,
    pub board: Board,
    pub last_sync: Option<NaiveDateTime>,
    pub sync_error: Option<String>,
    pub fired: BTreeSet<String>,
    pub image_cache: HashMap<String, String>,
    /// Last weather result and the place it was fetched for.
    pub weather: Option<(String, Weather)>,
    /// Files that could not be loaded, by path: the stamp that failed and
    /// why. They are skipped until saved again and shown as a sync error.
    pub broken: HashMap<String, (String, String)>,
}

pub struct AppState {
    inner: Mutex<Inner>,
    storage: Storage,
    syncing: AtomicBool,
}

/// Everything the frontend needs, in one payload. Mirrors `src/types.ts`.
#[derive(Debug, Clone, Serialize)]
pub struct BoardState {
    pub tips: Vec<Tip>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_up: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_error: Option<String>,
    pub settings: Settings,
    pub now: NaiveDateTime,
}

/// Local wall-clock time, truncated to whole seconds so it serialises as
/// `YYYY-MM-DDTHH:MM:SS`, the shape the frontend parses.
pub fn now() -> NaiveDateTime {
    let t = Local::now().naive_local();
    t.with_nanosecond(0).unwrap_or(t)
}

impl AppState {
    pub fn load(storage: Storage) -> Self {
        let inner = Inner {
            settings: storage.load_settings(),
            board: Board::new(storage.load_cache()),
            fired: storage.load_fired(),
            ..Default::default()
        };
        Self {
            inner: Mutex::new(inner),
            storage,
            syncing: AtomicBool::new(false),
        }
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Run `f` with the lock held. Poisoned locks are recovered.
    pub fn with<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> T {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }

    pub fn settings(&self) -> Settings {
        self.with(|i| i.settings.clone())
    }

    pub fn board(&self) -> Board {
        self.with(|i| i.board.clone())
    }

    pub fn snapshot(&self) -> BoardState {
        let now = now();
        self.with(|i| {
            let open: Vec<Tip> = ordered(&i.board.tips, now).into_iter().cloned().collect();
            let hidden: Vec<Tip> = i
                .board
                .tips
                .iter()
                .filter(|t| !open.iter().any(|o| o.id == t.id))
                .cloned()
                .collect();
            BoardState {
                next_up: next_up(&i.board.tips, now),
                tips: open.into_iter().chain(hidden).collect(),
                last_sync: i.last_sync,
                sync_error: i.sync_error.clone(),
                settings: i.settings.clone(),
                now,
            }
        })
    }

    pub fn emit<R: Runtime>(&self, app: &AppHandle<R>) {
        if let Err(e) = app.emit(EVENT_BOARD_UPDATED, self.snapshot()) {
            log::error!("emit failed: {e}");
        }
    }

    /// The tips folder from current settings.
    pub fn store(&self) -> Result<FolderStore, String> {
        let settings = self.settings();
        if !settings.is_configured() {
            return Err("Pick a tips folder in Settings first.".into());
        }
        FolderStore::open(Path::new(&settings.folder)).map_err(|e| e.to_string())
    }

    pub fn replace_board(&self, board: Board) {
        let tips = board.tips.clone();
        self.with(|i| i.board = board);
        if let Err(e) = self.storage.save_cache(&tips) {
            log::warn!("cache not saved: {e}");
        }
    }

    pub fn set_settings(&self, settings: Settings) -> Result<(), String> {
        self.storage.save_settings(&settings)?;
        self.with(|i| i.settings = settings);
        Ok(())
    }

    pub fn record_fired(&self, keys: impl IntoIterator<Item = String>, now: NaiveDateTime) {
        let fired = self.with(|i| {
            i.fired.extend(keys);
            i.fired = super::notify::prune_fired(&i.fired, now);
            i.fired.clone()
        });
        if let Err(e) = self.storage.save_fired(&fired) {
            log::warn!("fired set not saved: {e}");
        }
    }

    pub fn cache_image(&self, key: &str, data_url: &str) {
        self.with(|i| {
            if i.image_cache.len() >= IMAGE_CACHE_MAX {
                i.image_cache.clear();
            }
            i.image_cache.insert(key.to_string(), data_url.to_string());
        });
    }

    pub fn cached_image(&self, key: &str) -> Option<String> {
        self.with(|i| i.image_cache.get(key).cloned())
    }

    /// Weather for `place` if it was fetched less than `WEATHER_FRESH_MINUTES` ago.
    pub fn fresh_weather(&self, place: &str, now: NaiveDateTime) -> Option<Weather> {
        self.with(|i| {
            let (for_place, w) = i.weather.as_ref()?;
            let age = now - w.fetched_at;
            (for_place == place && age < chrono::Duration::minutes(WEATHER_FRESH_MINUTES))
                .then(|| w.clone())
        })
    }

    pub fn cache_weather(&self, place: &str, weather: Weather) {
        self.with(|i| i.weather = Some((place.to_string(), weather)));
    }

    /// Rescan the folder and read any changed files. Returns ids of tips
    /// that were not on the board before (empty on the very first load).
    /// The UI is only told when the board or the error state changed, so the
    /// frequent rescans cost nothing while the folder is quiet.
    pub async fn sync<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Vec<String>, String> {
        let Some(_running) = Running::start(&self.syncing) else {
            return Ok(Vec::new());
        };
        let result = self.sync_inner();
        let error = match &result {
            Err(e) => Some(e.clone()),
            Ok(_) => self.with(|i| broken_summary(&i.broken)),
        };
        let error_changed = self.with(|i| {
            let changed = i.sync_error != error;
            i.sync_error = error.clone();
            i.last_sync = Some(now());
            changed
        });
        if let (Some(e), true) = (&error, error_changed) {
            log::warn!("sync: {e}");
        }
        if matches!(&result, Ok((true, _))) || error_changed {
            self.emit(app);
        }
        result.map(|(_, new_ids)| new_ids)
    }

    /// `(board changed, ids of new tips)`.
    fn sync_inner(&self) -> Result<(bool, Vec<String>), String> {
        let store = self.store()?;
        // Board first, listing second: a tip created in between is then in
        // the listing (and read), never mistaken for a deleted one.
        let seen = self.board();
        let listing = store.list().map_err(|e| e.to_string())?;
        let stamps: HashMap<&str, &str> = listing
            .iter()
            .map(|f| (f.path.as_str(), f.stamp.as_str()))
            .collect();
        // A broken file is retried once it changes (or is gone).
        let broken = self.with(|i| {
            i.broken
                .retain(|path, (stamp, _)| stamps.get(path.as_str()) == Some(&stamp.as_str()));
            i.broken.clone()
        });
        let mut plan = seen.plan(&listing);
        plan.to_fetch.retain(|path| !broken.contains_key(path));
        if plan.to_fetch.is_empty() && plan.removed.is_empty() {
            return Ok((false, Vec::new()));
        }
        let mut fetched = Vec::with_capacity(plan.to_fetch.len());
        for path in &plan.to_fetch {
            let failed = match store.read_text(path) {
                Ok((text, stamp)) => match Tip::parse(path, Some(stamp.clone()), &text) {
                    Ok(tip) => {
                        fetched.push(tip);
                        continue;
                    }
                    Err(e) => (stamp, e.to_string()),
                },
                // Mid-write: the next scan reads the finished file.
                Err(FolderError::Changing(_)) => continue,
                Err(e) => (stamps[path.as_str()].to_string(), e.to_string()),
            };
            self.with(|i| i.broken.insert(path.clone(), failed));
        }
        let (changed, new_ids, tips) = self.with(|i| {
            // Apply against the *current* board, skipping any tip a command
            // wrote while the files above were read: that write is newer.
            let current = &i.board;
            let untouched = |path: &str| sha_of(current, path) == sha_of(&seen, path);
            let fetched: Vec<Tip> = fetched.into_iter().filter(|t| untouched(&t.path)).collect();
            let removed: Vec<String> = plan
                .removed
                .iter()
                .filter(|p| untouched(p))
                .cloned()
                .collect();
            let had_tips = !current.tips.is_empty();
            let (next, new_ids) = current.apply(fetched, &removed);
            let changed = next != *current;
            if changed {
                i.board = next;
                i.image_cache.clear();
            }
            let new_ids = if had_tips { new_ids } else { Vec::new() };
            (changed, new_ids, i.board.tips.clone())
        });
        if changed {
            if let Err(e) = self.storage.save_cache(&tips) {
                log::warn!("cache not saved: {e}");
            }
        }
        Ok((changed, new_ids))
    }

    /// Write a tip to its file and update the board with the new stamp.
    pub async fn save_tip<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        tip: Tip,
        message: &str,
    ) -> Result<Tip, String> {
        let text = tip.to_markdown().map_err(|e| e.to_string())?;
        self.save_tip_text(app, tip, &text, message).await
    }

    /// Like `save_tip`, but writes `text` verbatim (used for the new-tip
    /// template, whose comments the serialiser would drop).
    pub async fn save_tip_text<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        tip: Tip,
        text: &str,
        message: &str,
    ) -> Result<Tip, String> {
        let store = self.store()?;
        if let Some((_, reason)) = self.with(|i| i.broken.get(&tip.path).cloned()) {
            return Err(format!(
                "{} has an error; fix it in the editor and save first ({reason})",
                tip.path
            ));
        }
        log::info!("{message}");
        match store.write_text(&tip.path, text, tip.sha.as_deref()) {
            Ok(stamp) => {
                let saved = Tip {
                    sha: Some(stamp),
                    ..tip
                };
                self.replace_board(self.board().upsert(saved.clone()));
                self.emit(app);
                Ok(saved)
            }
            Err(e) => {
                // Usually a conflict: the file changed on disk. Reload it so
                // the board shows the current version, then report.
                let _ = self.sync(app).await;
                Err(e.to_string())
            }
        }
    }

    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> tauri::State<'_, AppState> {
        app.state::<AppState>()
    }
}

/// Clears the "sync running" flag when dropped, even on a panic.
struct Running<'a>(&'a AtomicBool);

impl<'a> Running<'a> {
    fn start(flag: &'a AtomicBool) -> Option<Self> {
        (!flag.swap(true, Ordering::SeqCst)).then_some(Self(flag))
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn sha_of<'a>(board: &'a Board, path: &str) -> Option<&'a Option<String>> {
    board.tips.iter().find(|t| t.path == path).map(|t| &t.sha)
}

/// One line about files that could not be loaded, for the sync dot.
fn broken_summary(broken: &HashMap<String, (String, String)>) -> Option<String> {
    let mut paths: Vec<&String> = broken.keys().collect();
    paths.sort();
    let first = paths.first()?;
    let (_, reason) = &broken[*first];
    let more = match paths.len() {
        1 => String::new(),
        n => format!(" (and {} more)", n - 1),
    };
    Some(format!("cannot load {first}: {reason}{more}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::folder::FolderStore;

    fn state_in(folder: &Path, data: &Path) -> AppState {
        let state = AppState::load(Storage::new(data.to_path_buf()));
        let settings = Settings {
            folder: folder.display().to_string(),
            ..Default::default()
        };
        state.with(|i| i.settings = settings);
        state
    }

    #[test]
    fn scan_loads_changes_and_reports_quiet_folders() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        let store = FolderStore::open(folder.path()).unwrap();
        store
            .write_text(
                "a.md",
                "---
title: A
---
",
                None,
            )
            .unwrap();

        let (changed, new_ids) = state.sync_inner().unwrap();
        assert!(changed);
        assert!(new_ids.is_empty(), "first load announces nothing");
        assert_eq!(state.board().get("a").unwrap().front.title, "A");

        assert_eq!(state.sync_inner().unwrap(), (false, vec![]));

        store
            .write_text(
                "b.md",
                "---
title: B
---
",
                None,
            )
            .unwrap();
        assert_eq!(state.sync_inner().unwrap(), (true, vec!["b".to_string()]));

        std::fs::remove_file(folder.path().join("a.md")).unwrap();
        assert!(state.sync_inner().unwrap().0);
        assert!(state.board().get("a").is_none());
    }

    #[test]
    fn broken_files_are_skipped_until_saved_again() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        std::fs::write(
            folder.path().join("bad.md"),
            "---
title: [unclosed
---
",
        )
        .unwrap();
        state.sync_inner().unwrap();
        assert!(state.board().get("bad").is_none());
        assert_eq!(state.sync_inner().unwrap(), (false, vec![]));
        let summary = state.with(|i| broken_summary(&i.broken)).unwrap();
        assert!(summary.starts_with("cannot load bad.md"), "{summary}");
        std::fs::write(
            folder.path().join("bad.md"),
            "---
title: Fixed now
---
",
        )
        .unwrap();
        state.sync_inner().unwrap();
        assert_eq!(state.board().get("bad").unwrap().front.title, "Fixed now");
        assert!(state.with(|i| i.broken.is_empty()));
    }

    #[test]
    fn missing_folder_is_reported() {
        let data = tempfile::tempdir().unwrap();
        let state = state_in(&data.path().join("gone"), data.path());
        assert!(state.sync_inner().is_err());
        let unset = AppState::load(Storage::new(data.path().to_path_buf()));
        assert!(unset.store().unwrap_err().contains("Settings"));
    }

    #[test]
    fn non_utf8_files_are_reported_not_retried() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        std::fs::write(folder.path().join("gbk.md"), [0xC4u8, 0xE3, 0xBA, 0xC3]).unwrap();
        state.sync_inner().unwrap();
        assert!(state.with(|i| i.broken.contains_key("gbk.md")));
        assert_eq!(state.sync_inner().unwrap(), (false, vec![]));
    }

    #[test]
    fn running_flag_resets_on_drop() {
        let flag = AtomicBool::new(false);
        {
            let _a = Running::start(&flag).unwrap();
            assert!(Running::start(&flag).is_none());
        }
        assert!(Running::start(&flag).is_some());
    }
}
