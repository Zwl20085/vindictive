//! Tauri commands: the only entry points the frontend can call.
//! Every mutating command returns the fresh `BoardState`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::Duration;
use tauri::{AppHandle, State};

use super::settings::{Dock, Language, Settings};
use super::state::{now, AppState, BoardState};
use super::{editor, secrets, windows};
use crate::core::capture;
use crate::core::template;
use crate::core::tip::{slugify, Kind, Tip};
use crate::sync::github::{validate_dir, GithubClient};
use crate::sync::meta;
use crate::sync::weather::{self, Weather};

const MAX_CAPTURE_CHARS: usize = 500;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MIN_SNOOZE_MINUTES: i64 = 1;
const MAX_SNOOZE_MINUTES: i64 = 60 * 24 * 30;
const MAX_LOG_CHARS: usize = 2000;
const TILE_SIZES: [&str; 3] = ["sm", "md", "wide"];

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> BoardState {
    state.snapshot()
}

#[tauri::command]
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Result<BoardState, String> {
    state.sync(&app).await?;
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn mark_done(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<BoardState, String> {
    let tip = find(&state, &id)?;
    let done = tip.complete(now());
    let msg = format!("vindictive: done \"{}\"", tip.front.title);
    state.save_tip(&app, done, &msg).await?;
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn reopen(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<BoardState, String> {
    let tip = find(&state, &id)?;
    let msg = format!("vindictive: reopen \"{}\"", tip.front.title);
    state.save_tip(&app, tip.reopen(), &msg).await?;
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn snooze(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    minutes: i64,
) -> Result<BoardState, String> {
    if !(MIN_SNOOZE_MINUTES..=MAX_SNOOZE_MINUTES).contains(&minutes) {
        return Err(format!(
            "snooze must be {MIN_SNOOZE_MINUTES}..{MAX_SNOOZE_MINUTES} minutes"
        ));
    }
    let tip = find(&state, &id)?;
    let until = now() + Duration::minutes(minutes);
    let msg = format!("vindictive: snooze \"{}\"", tip.front.title);
    state.save_tip(&app, tip.snooze_until(until), &msg).await?;
    Ok(state.snapshot())
}

/// Set or clear (`None`) the tile colour override.
#[tauri::command]
pub async fn set_color(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    color: Option<String>,
) -> Result<BoardState, String> {
    let color = color.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if let Some(c) = &color {
        if !is_hex_color(c) {
            return Err(format!("{c:?} is not a hex colour like #4A3B6B"));
        }
    }
    let tip = find(&state, &id)?;
    let mut front = tip.front.clone();
    front.color = color;
    let msg = format!("vindictive: colour \"{}\"", tip.front.title);
    state.save_tip(&app, tip.with_front(front), &msg).await?;
    Ok(state.snapshot())
}

/// Set or clear (`None`) the tile size override (`sm`, `md`, `wide`).
#[tauri::command]
pub async fn set_size(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    size: Option<String>,
) -> Result<BoardState, String> {
    let size = size.map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty());
    if let Some(s) = &size {
        if !TILE_SIZES.contains(&s.as_str()) {
            return Err(format!("size must be one of {}", TILE_SIZES.join(", ")));
        }
    }
    let tip = find(&state, &id)?;
    let mut front = tip.front.clone();
    front.size = size;
    let msg = format!("vindictive: size \"{}\"", tip.front.title);
    state.save_tip(&app, tip.with_front(front), &msg).await?;
    Ok(state.snapshot())
}

/// Set or clear (`None`) the manual board position written by drag and drop.
#[tauri::command]
pub async fn set_order(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    order: Option<f64>,
) -> Result<BoardState, String> {
    if let Some(o) = order {
        if !o.is_finite() {
            return Err("order must be a finite number".into());
        }
    }
    let tip = find(&state, &id)?;
    let mut front = tip.front.clone();
    front.order = order;
    let msg = format!("vindictive: move \"{}\"", tip.front.title);
    state.save_tip(&app, tip.with_front(front), &msg).await?;
    Ok(state.snapshot())
}

/// Commit every pending local edit now.
#[tauri::command]
pub async fn push_now(app: AppHandle, state: State<'_, AppState>) -> Result<BoardState, String> {
    editor::push_pending(&app, &state).await;
    Ok(state.snapshot())
}

/// Resize the board window to `height` logical pixels, clamped to the
/// monitor's work area, keeping it docked.
#[tauri::command]
pub fn fit_window(app: AppHandle, state: State<'_, AppState>, height: f64) -> Result<(), String> {
    if !height.is_finite() || height <= 0.0 {
        return Err("bad height".into());
    }
    windows::fit_main(&app, state.settings().dock, height)
}

fn is_hex_color(value: &str) -> bool {
    let hex = match value.strip_prefix('#') {
        Some(h) => h,
        None => return false,
    };
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Open the tip in the local editor and keep it in sync; returns the file path.
#[tauri::command]
pub fn edit_local(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<String, String> {
    editor::open(&app, &state, &id).map(|p| p.display().to_string())
}

/// Delete a tip's file from the repository and drop it from the board.
#[tauri::command]
pub async fn delete_tip(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<BoardState, String> {
    let tip = find(&state, &id)?;
    let sha = tip
        .sha
        .clone()
        .ok_or("this tip has not been synced yet; try again after the next sync")?;
    let client = state.client()?;
    let msg = format!("vindictive: delete \"{}\"", tip.front.title);
    match client.delete_file(&tip.path, &sha, &msg).await {
        Ok(()) => {
            state.replace_board(state.board().remove(&id));
            state.emit(&app);
            Ok(state.snapshot())
        }
        Err(e) => {
            let _ = state.sync(&app).await;
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn create_tip(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<BoardState, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("nothing to capture".into());
    }
    if text.chars().count() > MAX_CAPTURE_CHARS {
        return Err(format!(
            "keep quick captures under {MAX_CAPTURE_CHARS} characters"
        ));
    }
    let created_at = now();
    let front = capture::parse(text, created_at).ok_or("could not find a title in that line")?;
    let settings = state.settings();
    let stem = format!(
        "{}-{}",
        created_at.format("%Y-%m-%d"),
        slugify(&front.title)
    );
    let file_name = state.board().unique_file_name(&stem);
    let path = settings.tip_path(&file_name);
    let title = front.title.clone();
    let zh = settings.language == Language::Zh;
    let (tip, text) = template::render(&path, front, zh).map_err(|e| e.to_string())?;
    let msg = format!("vindictive: capture \"{title}\"");
    let saved = state.save_tip_text(&app, tip, &text, &msg).await?;
    if saved.front.arxiv.is_some() || saved.front.doi.is_some() {
        let app2 = app.clone();
        let id = saved.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = enrich(&app2, &id).await {
                log::warn!("auto enrich failed for {id}: {e}");
            }
        });
    }
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<BoardState, String> {
    let clean = settings.validated()?;
    let previous = state.settings();
    state.set_settings(clean.clone())?;
    windows::apply_settings(&app, &previous, &clean);
    let repo_changed = previous.repo_ref() != clean.repo_ref() || previous.dir != clean.dir;
    if repo_changed {
        state.replace_board(Default::default());
    }
    state.emit(&app);
    if repo_changed && clean.is_configured() {
        let _ = state.sync(&app).await;
    }
    Ok(state.snapshot())
}

#[tauri::command]
pub fn set_token(app: AppHandle, state: State<'_, AppState>, token: String) -> Result<(), String> {
    secrets::set_token(&token)?;
    state.emit(&app);
    Ok(())
}

#[tauri::command]
pub fn clear_token(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    secrets::clear_token()?;
    state.emit(&app);
    Ok(())
}

#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>) -> Result<String, String> {
    let client = state.client()?;
    let settings = state.settings();
    let dir = validate_dir(&settings.dir).map_err(|e| e.to_string())?;
    let files = client
        .list_dir_or_empty(&dir)
        .await
        .map_err(|e| e.to_string())?;
    let count = files
        .iter()
        .filter(|f| f.kind == "file" && f.name.ends_with(".md"))
        .count();
    let repo = client.repo();
    if files.is_empty() {
        return Ok(format!(
            "OK: {}/{} is reachable. No tips in {}/ yet; your first tip creates it.",
            repo.owner, repo.repo, settings.dir
        ));
    }
    Ok(format!(
        "OK: {count} tips in {}/{}/{}",
        repo.owner, repo.repo, settings.dir
    ))
}

#[tauri::command]
pub async fn fetch_image(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let path = path.trim();
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("data:") {
        return Ok(path.to_string());
    }
    let settings = state.settings();
    let repo_path = resolve_repo_path(&settings, path)?;
    if let Some(cached) = state.cached_image(&repo_path) {
        return Ok(cached);
    }
    let client = state.client()?;
    let bytes = client
        .get_bytes(&repo_path)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("image is larger than 8 MB".into());
    }
    let data_url = format!(
        "data:{};base64,{}",
        mime_for(&repo_path),
        B64.encode(&bytes)
    );
    state.cache_image(&repo_path, &data_url);
    Ok(data_url)
}

#[tauri::command]
pub async fn enrich_tip(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<BoardState, String> {
    enrich(&app, &id).await?;
    Ok(state.snapshot())
}

/// Current weather for the configured place, `None` when weather is off.
/// Results are cached for a while so the panel can ask freely.
#[tauri::command]
pub async fn fetch_weather(state: State<'_, AppState>) -> Result<Option<Weather>, String> {
    let place = state.settings().weather_location.trim().to_string();
    if place.is_empty() {
        return Ok(None);
    }
    let now = now();
    if let Some(cached) = state.fresh_weather(&place, now) {
        return Ok(Some(cached));
    }
    let fetched = weather::fetch(&place, now).await.map_err(|e| e.to_string())?;
    state.cache_weather(&place, fetched.clone());
    Ok(Some(fetched))
}

#[tauri::command]
pub fn show_capture(app: AppHandle) -> Result<(), String> {
    windows::show_capture(&app)
}

#[tauri::command]
pub fn hide_capture(app: AppHandle) -> Result<(), String> {
    windows::hide_capture(&app)
}

#[tauri::command]
pub fn dock_window(app: AppHandle, state: State<'_, AppState>, dock: Dock) -> Result<(), String> {
    windows::dock_main(&app, dock)?;
    let settings = Settings {
        dock,
        ..state.settings()
    };
    state.set_settings(settings)?;
    state.emit(&app);
    Ok(())
}

/// Frontend errors land in the same log file as backend ones.
#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    let message: String = message.chars().take(MAX_LOG_CHARS).collect();
    match level.as_str() {
        "error" => log::error!("[ui] {message}"),
        "warn" => log::warn!("[ui] {message}"),
        _ => log::info!("[ui] {message}"),
    }
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

fn find(state: &State<'_, AppState>, id: &str) -> Result<Tip, String> {
    state
        .board()
        .get(id)
        .cloned()
        .ok_or_else(|| format!("tip {id} not found"))
}

/// Fetch paper metadata for a tip with an arXiv id or DOI.
pub async fn enrich(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = AppState::from_app(app);
    let tip = find(&state, id)?;
    let paper = if let Some(arxiv) = &tip.front.arxiv {
        meta::fetch_arxiv(arxiv).await.map_err(|e| e.to_string())?
    } else if let Some(doi) = &tip.front.doi {
        meta::fetch_crossref(doi).await.map_err(|e| e.to_string())?
    } else {
        return Err("tip has neither an arXiv id nor a DOI".into());
    };
    // Re-read: the tip may have been completed or snoozed while we waited.
    let current = find(&state, id)?;
    let mut front = current.front.clone();
    front.paper = Some(paper);
    if front.kind == Kind::Task {
        front.kind = Kind::Reading;
    }
    let enriched = current.with_front(front);
    let msg = format!("vindictive: paper metadata for \"{}\"", current.front.title);
    state.save_tip(app, enriched, &msg).await.map(|_| ())
}

/// Resolve an image reference to a repository path.
fn resolve_repo_path(settings: &Settings, path: &str) -> Result<String, String> {
    let cleaned = validate_dir(path).map_err(|e| e.to_string())?;
    if cleaned.is_empty() {
        return Err("empty image path".into());
    }
    let dir = settings.dir.as_str();
    if dir.is_empty() || cleaned.starts_with(&format!("{dir}/")) {
        Ok(cleaned)
    } else {
        Ok(format!("{dir}/{cleaned}"))
    }
}

fn mime_for(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

#[allow(dead_code)]
fn _assert_client_send(c: GithubClient) -> impl Send {
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_image_paths() {
        let s = Settings {
            dir: "tips".into(),
            ..Default::default()
        };
        assert_eq!(
            resolve_repo_path(&s, "figures/a.png").unwrap(),
            "tips/figures/a.png"
        );
        assert_eq!(
            resolve_repo_path(&s, "tips/figures/a.png").unwrap(),
            "tips/figures/a.png"
        );
        assert!(resolve_repo_path(&s, "./figures/a.png").is_err());
        assert!(resolve_repo_path(&s, "../secret.png").is_err());
        assert!(resolve_repo_path(&s, "").is_err());
        let root = Settings {
            dir: "".into(),
            ..Default::default()
        };
        assert_eq!(resolve_repo_path(&root, "a.png").unwrap(), "a.png");
    }

    #[test]
    fn hex_colours() {
        assert!(is_hex_color("#4A3B6B"));
        assert!(is_hex_color("#abc"));
        assert!(!is_hex_color("4A3B6B"));
        assert!(!is_hex_color("#12345"));
        assert!(!is_hex_color("#GGGGGG"));
    }

    #[test]
    fn mime_types() {
        assert_eq!(mime_for("x/y.PNG"), "image/png");
        assert_eq!(mime_for("a.jpeg"), "image/jpeg");
        assert_eq!(mime_for("noext"), "application/octet-stream");
    }
}
