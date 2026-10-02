//! Background loop: rescan the tips folder and raise reminder toasts.

use std::time::Duration;

use tauri::AppHandle;

use super::notify;
use super::state::{now, AppState};

/// How often the tips folder is rescanned. A scan of an unchanged folder is
/// one directory listing, so this can be short: edits made in an editor or
/// arriving through OneDrive show up within a few seconds.
const TICK: Duration = Duration::from_secs(3);
const MAX_NEW_TIP_TITLES: usize = 3;

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Give the webview a moment before the first scan.
        tokio::time::sleep(Duration::from_secs(1)).await;
        tick(&app).await;
        // Always send the first scan's board: a quiet scan emits nothing, and
        // the window may have asked for state before the backend was ready.
        AppState::from_app(&app).emit(&app);
        loop {
            tokio::time::sleep(TICK).await;
            tick(&app).await;
        }
    });
}

async fn tick(app: &AppHandle) {
    let state = AppState::from_app(app);
    if state.settings().is_configured() {
        match state.sync(app).await {
            Ok(new_ids) => announce_new_tips(app, &state, &new_ids),
            Err(e) => log::debug!("scheduled sync skipped: {e}"),
        }
    }
    fire_reminders(app, &state);
}

fn announce_new_tips(app: &AppHandle, state: &AppState, new_ids: &[String]) {
    if new_ids.is_empty() || !state.settings().notify_new_tips {
        return;
    }
    let board = state.board();
    let titles: Vec<String> = new_ids
        .iter()
        .filter_map(|id| board.get(id))
        .filter(|t| t.is_open())
        .map(|t| t.display_title().to_string())
        .collect();
    match titles.as_slice() {
        [] => {}
        [one] => notify::show(app, "New tip", one),
        many => {
            let shown: Vec<&str> = many
                .iter()
                .take(MAX_NEW_TIP_TITLES)
                .map(String::as_str)
                .collect();
            let more = many.len().saturating_sub(shown.len());
            let body = if more > 0 {
                format!("{} · +{more} more", shown.join(" · "))
            } else {
                shown.join(" · ")
            };
            notify::show(app, &format!("{} new tips", many.len()), &body);
        }
    }
}

fn fire_reminders(app: &AppHandle, state: &AppState) {
    let at = now();
    let toasts = state.with(|i| notify::due_reminders(&i.board.tips, &i.fired, at));
    if toasts.is_empty() {
        return;
    }
    for t in &toasts {
        notify::show(app, &t.title, &t.body);
    }
    state.record_fired(toasts.into_iter().map(|t| t.key), at);
}
