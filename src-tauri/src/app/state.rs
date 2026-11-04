//! Shared application state and the sync engine.
//!
//! Locking discipline: the mutex is held only for short, non-async sections.
//! File reads and writes always happen on a cloned snapshot.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::{Local, NaiveDateTime, Timelike};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::board::{Board, SyncPlan};
use super::settings::Settings;
use super::storage::Storage;
use crate::core::nextup::{next_up, ordered};
use crate::core::tip::Tip;
use crate::sync::folder::{conflict_summary, FileEntry, FolderError, FolderStore};
use crate::sync::weather::Weather;

pub const EVENT_BOARD_UPDATED: &str = "board-updated";
const IMAGE_CACHE_MAX: usize = 64;
/// More new tips than this in one scan is a bulk arrival (a folder that came
/// back, a big copy), not something to toast about tip by tip.
const MAX_ANNOUNCED_NEW_TIPS: usize = 10;
/// From this many new tips on, they are also a bulk arrival when they make
/// up more than half of the board.
const BULK_MIN_NEW_TIPS: usize = 3;
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
    /// The last scan found the folder empty while the board had tips. The
    /// removal is applied only if the next scan is empty too, so a folder
    /// that briefly vanishes (OneDrive unmounting) does not wipe the board.
    pub pending_wipe: bool,
    /// OneDrive conflict copies in the last listing, as one line.
    pub conflicts: Option<String>,
}

pub struct AppState {
    inner: Mutex<Inner>,
    storage: Storage,
    syncing: AtomicBool,
    /// A sync was asked for while one was running; it scans once more.
    rescan: AtomicBool,
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
            rescan: AtomicBool::new(false),
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

    /// A full copy of the board. Prefer `tip` or `with` for lookups.
    pub fn board(&self) -> Board {
        self.with(|i| i.board.clone())
    }

    /// A copy of one tip, without copying the rest of the board.
    pub fn tip(&self, id: &str) -> Option<Tip> {
        self.with(|i| i.board.get(id).cloned())
    }

    pub fn snapshot(&self) -> BoardState {
        let now = now();
        self.with(|i| {
            let open = ordered(&i.board.tips, now);
            let open_ids: HashSet<&str> = open.iter().map(|t| t.id.as_str()).collect();
            let hidden = i
                .board
                .tips
                .iter()
                .filter(|t| !open_ids.contains(t.id.as_str()));
            BoardState {
                next_up: next_up(&i.board.tips, now),
                tips: open.iter().copied().chain(hidden).cloned().collect(),
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
        self.update_board(|_| board);
    }

    /// Replace the board with `f(current board)` under one lock, so the
    /// change cannot race with a sync applying its own result in between.
    pub fn update_board(&self, f: impl FnOnce(&Board) -> Board) {
        let tips = self.with(|i| {
            i.board = f(&i.board);
            i.board.tips.clone()
        });
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
    ///
    /// A call made while another sync runs returns at once, and the running
    /// sync scans once more afterwards, so the request is never lost.
    pub async fn sync<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Vec<String>, String> {
        coalesced(&self.syncing, &self.rescan, || self.sync_once(app))
    }

    fn sync_once<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Vec<String>, String> {
        let result = self.sync_inner();
        let error = match &result {
            Err(e) => Some(e.clone()),
            Ok(_) => self.with(|i| sync_summary(&i.broken, i.conflicts.as_deref())),
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
        // the listing (and read), never mistaken for a deleted one. Only the
        // path -> stamp pairs are taken, not the whole board.
        let seen: Vec<(String, Option<String>)> = self.with(|i| {
            i.board
                .tips
                .iter()
                .map(|t| (t.path.clone(), t.sha.clone()))
                .collect()
        });
        let listing = store.list().map_err(|e| e.to_string())?;
        // The copies stay on the board as tips; the summary only flags them.
        let conflicts = conflict_summary(&listing);
        self.with(|i| i.conflicts = conflicts);
        if listing.is_empty() && !seen.is_empty() {
            // Apply an empty folder only when two scans in a row agree.
            let confirmed = self.with(|i| {
                let confirmed = i.pending_wipe;
                i.pending_wipe = !confirmed;
                confirmed
            });
            if !confirmed {
                log::warn!(
                    "tips folder is empty but the board has {} tips; waiting for the next scan",
                    seen.len()
                );
                return Ok((false, Vec::new()));
            }
        } else {
            self.with(|i| i.pending_wipe = false);
        }
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
        let mut plan = plan(&seen, &listing);
        plan.to_fetch.retain(|path| !broken.contains_key(path));
        if plan.to_fetch.is_empty() && plan.removed.is_empty() {
            return Ok((false, Vec::new()));
        }
        // First tip per path wins, as `Iterator::find` did before.
        let seen: HashMap<&str, &Option<String>> = seen
            .iter()
            .rev()
            .map(|(p, sha)| (p.as_str(), sha))
            .collect();
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
            let now_shas: HashMap<&str, &Option<String>> = current
                .tips
                .iter()
                .rev()
                .map(|t| (t.path.as_str(), &t.sha))
                .collect();
            let untouched = |path: &str| now_shas.get(path) == seen.get(path);
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
            let new_ids = if !had_tips {
                Vec::new()
            } else if is_bulk(new_ids.len(), i.board.tips.len()) {
                log::info!("{} new tips at once; not announcing them", new_ids.len());
                Vec::new()
            } else {
                new_ids
            };
            let tips = if changed {
                i.board.tips.clone()
            } else {
                Vec::new()
            };
            (changed, new_ids, tips)
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
    /// template, whose comments the serialiser would drop). `message` is the
    /// action for the log ("done", "edit"); it must not hold tip content.
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
        log::info!("vindictive: {message} {}", tip.id);
        let written = {
            let (path, text, base) = (tip.path.clone(), text.to_string(), tip.sha.clone());
            tauri::async_runtime::spawn_blocking(move || {
                store
                    .write_text(&path, &text, base.as_deref())
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| format!("write failed: {e}"))?
        };
        match written {
            Ok(stamp) => {
                let saved = Tip {
                    sha: Some(stamp),
                    ..tip
                };
                let upserted = saved.clone();
                self.update_board(move |board| board.upsert(upserted));
                self.emit(app);
                Ok(saved)
            }
            Err(e) => {
                // Usually a conflict: the file changed on disk. Reload it so
                // the board shows the current version, then report.
                let _ = self.sync(app).await;
                Err(e)
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

/// Run `once` unless a run is already going; then ask that run to go again
/// instead. New ids from every pass are returned together.
fn coalesced(
    running: &AtomicBool,
    rescan: &AtomicBool,
    mut once: impl FnMut() -> Result<Vec<String>, String>,
) -> Result<Vec<String>, String> {
    let mut new_ids = Vec::new();
    loop {
        let Some(guard) = Running::start(running) else {
            rescan.store(true, Ordering::SeqCst);
            return Ok(new_ids);
        };
        rescan.store(false, Ordering::SeqCst);
        new_ids.extend(once()?);
        drop(guard);
        // A request came in while we ran: go again (or, if another caller
        // has started meanwhile, leave the request to it).
        if !rescan.swap(false, Ordering::SeqCst) {
            return Ok(new_ids);
        }
    }
}

/// What to read and what is gone, from the board's `(path, stamp)` pairs
/// and the folder listing. Same result as `Board::plan`, without needing a
/// copy of the board.
fn plan(seen: &[(String, Option<String>)], files: &[FileEntry]) -> SyncPlan {
    let known: HashMap<&str, &str> = seen
        .iter()
        .map(|(p, sha)| (p.as_str(), sha.as_deref().unwrap_or("")))
        .collect();
    let to_fetch = files
        .iter()
        .filter(|f| known.get(f.path.as_str()) != Some(&f.stamp.as_str()))
        .map(|f| f.path.clone())
        .collect();
    let present: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let removed = seen
        .iter()
        .filter(|(p, _)| !present.contains(p.as_str()))
        .map(|(p, _)| p.clone())
        .collect();
    SyncPlan { to_fetch, removed }
}

/// `new` tips arriving on a board of `total` (counting them) are a bulk
/// arrival: too many to toast one by one.
fn is_bulk(new: usize, total: usize) -> bool {
    new > MAX_ANNOUNCED_NEW_TIPS || (new >= BULK_MIN_NEW_TIPS && new * 2 > total)
}

/// Broken files and conflict copies together, for the sync dot.
fn sync_summary(
    broken: &HashMap<String, (String, String)>,
    conflicts: Option<&str>,
) -> Option<String> {
    match (broken_summary(broken), conflicts) {
        (Some(b), Some(c)) => Some(format!("{b}; {c}")),
        (b, c) => b.or_else(|| c.map(str::to_string)),
    }
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

    fn write_tip(folder: &Path, name: &str, title: &str) {
        std::fs::write(folder.join(name), format!("---\ntitle: {title}\n---\n")).unwrap();
    }

    #[test]
    fn empty_folder_wipes_only_after_two_scans() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        write_tip(folder.path(), "a.md", "A");
        write_tip(folder.path(), "b.md", "B");
        state.sync_inner().unwrap();

        // The folder blinks out for one scan and comes back: nothing changes
        // and nothing is announced as new.
        std::fs::remove_file(folder.path().join("a.md")).unwrap();
        std::fs::remove_file(folder.path().join("b.md")).unwrap();
        assert_eq!(state.sync_inner().unwrap(), (false, vec![]));
        assert_eq!(state.with(|i| i.board.tips.len()), 2);
        write_tip(folder.path(), "a.md", "A");
        write_tip(folder.path(), "b.md", "B");
        let (_, new_ids) = state.sync_inner().unwrap();
        assert!(new_ids.is_empty(), "{new_ids:?}");
        assert!(!state.with(|i| i.pending_wipe));

        // Empty on two scans in a row: really emptied.
        std::fs::remove_file(folder.path().join("a.md")).unwrap();
        std::fs::remove_file(folder.path().join("b.md")).unwrap();
        assert_eq!(state.sync_inner().unwrap(), (false, vec![]));
        assert!(state.sync_inner().unwrap().0);
        assert!(state.with(|i| i.board.tips.is_empty()));
        assert!(!state.with(|i| i.pending_wipe));
    }

    #[test]
    fn bulk_arrivals_are_not_announced() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        write_tip(folder.path(), "a.md", "A");
        write_tip(folder.path(), "b.md", "B");
        state.sync_inner().unwrap();

        // Three new tips on a board of two: more than half, so a bulk arrival.
        for n in 0..3 {
            write_tip(folder.path(), &format!("x{n}.md"), "X");
        }
        let (changed, new_ids) = state.sync_inner().unwrap();
        assert!(changed);
        assert!(new_ids.is_empty(), "{new_ids:?}");

        // One more is announced as usual.
        write_tip(folder.path(), "c.md", "C");
        assert_eq!(state.sync_inner().unwrap(), (true, vec!["c".to_string()]));

        assert!(is_bulk(11, 1000));
        assert!(!is_bulk(10, 1000));
        assert!(!is_bulk(2, 3));
        assert!(is_bulk(3, 5));
        assert!(!is_bulk(3, 6));
    }

    #[test]
    fn plan_matches_board_plan() {
        let tips: Vec<Tip> = [("a.md", "1"), ("b.md", "2"), ("c.md", "3")]
            .iter()
            .map(|(p, sha)| Tip::parse(p, Some(sha.to_string()), "---\ntitle: T\n---\n").unwrap())
            .collect();
        let board = Board::new(tips);
        let listing: Vec<FileEntry> = [("a.md", "1"), ("b.md", "changed"), ("d.md", "4")]
            .iter()
            .map(|(p, s)| FileEntry {
                path: p.to_string(),
                stamp: s.to_string(),
            })
            .collect();
        let seen: Vec<(String, Option<String>)> = board
            .tips
            .iter()
            .map(|t| (t.path.clone(), t.sha.clone()))
            .collect();
        assert_eq!(plan(&seen, &listing), board.plan(&listing));
    }

    #[test]
    fn sync_requested_while_running_runs_again() {
        let running = AtomicBool::new(false);
        let rescan = AtomicBool::new(false);
        let mut passes = 0;
        let ids = coalesced(&running, &rescan, || {
            passes += 1;
            if passes == 1 {
                // Someone asks for a sync while this pass runs.
                assert_eq!(coalesced(&running, &rescan, || unreachable!()), Ok(vec![]));
            }
            Ok(vec![format!("p{passes}")])
        })
        .unwrap();
        assert_eq!(passes, 2);
        assert_eq!(ids, vec!["p1", "p2"]);
        assert!(!running.load(Ordering::SeqCst));
        assert!(!rescan.load(Ordering::SeqCst));
    }

    #[test]
    fn upsert_and_lookup_without_copying_the_board() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        let tip = Tip::parse("a.md", Some("1".into()), "---\ntitle: A\n---\n").unwrap();
        state.update_board(|b| b.upsert(tip.clone()));
        assert_eq!(state.tip("a"), Some(tip));
        assert_eq!(state.storage().load_cache().len(), 1);
        assert!(state.tip("missing").is_none());
    }

    #[test]
    fn conflict_copies_are_flagged_and_kept() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = state_in(folder.path(), data.path());
        write_tip(folder.path(), "a.md", "A");
        write_tip(folder.path(), "a-DESKTOP-AB12CD3.md", "A");
        state.sync_inner().unwrap();
        let conflicts = state.with(|i| i.conflicts.clone());
        let summary = state.with(|i| sync_summary(&i.broken, i.conflicts.as_deref()));
        assert_eq!(summary, conflicts);
        assert!(conflicts
            .unwrap()
            .contains("a-DESKTOP-AB12CD3.md (of a.md)"));
        assert_eq!(
            state.with(|i| i.board.tips.len()),
            2,
            "copies are not hidden"
        );
        let broken: HashMap<String, (String, String)> =
            [("b.md".to_string(), ("s".to_string(), "bad".to_string()))].into();
        assert_eq!(
            sync_summary(&broken, Some("c")).as_deref(),
            Some("cannot load b.md: bad; c")
        );
        assert_eq!(sync_summary(&HashMap::new(), None), None);
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
