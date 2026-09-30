//! Shared application state and the sync engine.
//!
//! Locking discipline: the mutex is held only for short, non-async sections.
//! Network calls always happen on a cloned snapshot.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::{Local, NaiveDateTime, Timelike};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::board::Board;
use super::editor::EditSession;
use super::secrets;
use super::settings::Settings;
use super::storage::Storage;
use crate::core::nextup::{next_up, ordered};
use crate::core::tip::Tip;
use crate::sync::github::{GithubClient, GithubError};
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
    /// Files open in the local editor, by tip id.
    pub edits: HashMap<String, EditSession>,
    /// When local edits were last pushed (scheduled or manual).
    pub last_push: Option<NaiveDateTime>,
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
    pub has_token: bool,
    pub settings: Settings,
    pub now: NaiveDateTime,
    /// Local edits saved but not yet committed.
    pub pending_edits: usize,
    /// When the next scheduled push will run, if edits are pending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_push: Option<NaiveDateTime>,
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
            edits: storage
                .load_edits()
                .into_iter()
                .map(|s| (s.id.clone(), s))
                .collect(),
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
        let has_token = secrets::get_token().ok().flatten().is_some();
        self.with(|i| {
            let open: Vec<Tip> = ordered(&i.board.tips, now).into_iter().cloned().collect();
            let hidden: Vec<Tip> = i
                .board
                .tips
                .iter()
                .filter(|t| !open.iter().any(|o| o.id == t.id))
                .cloned()
                .collect();
            let pending_edits = i.edits.values().filter(|s| s.pending.is_some()).count();
            let next_push = if pending_edits > 0 {
                super::editor::next_push_at(i.settings.push_interval_minutes, i.last_push)
            } else {
                None
            };
            BoardState {
                next_up: next_up(&i.board.tips, now),
                tips: open.into_iter().chain(hidden).collect(),
                last_sync: i.last_sync,
                sync_error: i.sync_error.clone(),
                has_token,
                settings: i.settings.clone(),
                now,
                pending_edits,
                next_push,
            }
        })
    }

    pub fn emit<R: Runtime>(&self, app: &AppHandle<R>) {
        if let Err(e) = app.emit(EVENT_BOARD_UPDATED, self.snapshot()) {
            log::error!("emit failed: {e}");
        }
    }

    /// Build a GitHub client from current settings and the stored token.
    pub fn client(&self) -> Result<GithubClient, String> {
        let settings = self.settings();
        if !settings.is_configured() {
            return Err("Set the tips repository in Settings first.".into());
        }
        let token = secrets::get_token()?.ok_or("Add a GitHub token in Settings first.")?;
        GithubClient::new(settings.repo_ref(), &token).map_err(|e| e.to_string())
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

    /// Mutate the editor sessions and persist them.
    pub fn update_edits(&self, f: impl FnOnce(&mut HashMap<String, EditSession>)) {
        let snapshot: Vec<EditSession> = self.with(|i| {
            f(&mut i.edits);
            i.edits.values().cloned().collect()
        });
        if let Err(e) = self.storage.save_edits(&snapshot) {
            log::warn!("edit sessions not saved: {e}");
        }
    }

    /// Pull the remote listing and any changed files. Returns ids of tips
    /// that were not on the board before (empty on the very first load).
    pub async fn sync<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Vec<String>, String> {
        if self.syncing.swap(true, Ordering::SeqCst) {
            return Ok(Vec::new());
        }
        let result = self.sync_inner().await;
        self.syncing.store(false, Ordering::SeqCst);
        match &result {
            Ok(_) => self.with(|i| {
                i.sync_error = None;
                i.last_sync = Some(now());
            }),
            Err(e) => {
                log::warn!("sync failed: {e}");
                self.with(|i| i.sync_error = Some(e.clone()));
            }
        }
        self.emit(app);
        result
    }

    async fn sync_inner(&self) -> Result<Vec<String>, String> {
        let client = self.client()?;
        let (dir, board) = self.with(|i| (i.settings.dir.clone(), i.board.clone()));
        let listing = client
            .list_dir_or_empty(&dir)
            .await
            .map_err(|e| e.to_string())?;
        let plan = board.plan(&listing);
        let mut fetched = Vec::with_capacity(plan.to_fetch.len());
        for path in &plan.to_fetch {
            let (text, sha) = client.get_text(path).await.map_err(|e| e.to_string())?;
            match Tip::parse(path, Some(sha), &text) {
                Ok(tip) => fetched.push(tip),
                Err(e) => log::warn!("skipping {path}: {e}"),
            }
        }
        // Merge into the *current* board: commands may have changed it while
        // the network calls above were in flight.
        let current = self.board();
        let had_tips = !current.tips.is_empty();
        let (next, new_ids) = current.apply(fetched, &plan.removed);
        self.replace_board(next);
        Ok(if had_tips { new_ids } else { Vec::new() })
    }

    /// Write a tip back to GitHub and update the board with its new SHA.
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
        let client = self.client()?;
        // Optimistic local update so the UI reacts immediately.
        self.replace_board(self.board().upsert(tip.clone()));
        self.emit(app);
        match client
            .put_text(&tip.path, text, tip.sha.as_deref(), message)
            .await
        {
            Ok(sha) => {
                let saved = Tip {
                    sha: Some(sha),
                    ..tip
                };
                self.replace_board(self.board().upsert(saved.clone()));
                self.emit(app);
                Ok(saved)
            }
            Err(GithubError::Conflict { path }) => {
                let _ = self.sync(app).await;
                Err(GithubError::Conflict { path }.to_string())
            }
            Err(e) => {
                let _ = self.sync(app).await;
                Err(e.to_string())
            }
        }
    }

    pub fn from_app<R: Runtime>(app: &AppHandle<R>) -> tauri::State<'_, AppState> {
        app.state::<AppState>()
    }
}
