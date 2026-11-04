//! Tauri commands: the only entry points the frontend can call.
//! Every mutating command returns the fresh `BoardState`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDateTime};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use super::settings::{Dock, Language, Settings};
use super::state::{now, AppState, BoardState};
use super::{editor, hotkeys, windows};
use crate::core::capture;
use crate::core::edit::{self, TipEdit};
use crate::core::template;
use crate::core::timeparse::format_dt;
use crate::core::tip::{slugify, FrontMatter, Kind, Tip};
use crate::sync::folder::{FolderError, FolderStore};
use crate::sync::meta;
use crate::sync::weather::{self, Weather};

const MAX_CAPTURE_CHARS: usize = 500;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MIN_SNOOZE_MINUTES: i64 = 1;
const MAX_SNOOZE_MINUTES: i64 = 60 * 24 * 30;
const MAX_LOG_CHARS: usize = 2000;

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
    state.save_tip(&app, done, "done").await?;
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn reopen(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<BoardState, String> {
    let tip = find(&state, &id)?;
    state.save_tip(&app, tip.reopen(), "reopen").await?;
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
    state
        .save_tip(&app, tip.snooze_until(until), "snooze")
        .await?;
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
    let color = color
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    if let Some(c) = &color {
        if !edit::is_hex_color(c) {
            return Err(format!("{c:?} is not a hex colour like #4A3B6B"));
        }
    }
    let tip = find(&state, &id)?;
    let mut front = tip.front.clone();
    front.color = color;
    state
        .save_tip(&app, tip.with_front(front), "colour")
        .await?;
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
    let size = size
        .filter(|s| !s.trim().is_empty())
        .map(|s| edit::tile_size(&s))
        .transpose()?;
    let tip = find(&state, &id)?;
    let mut front = tip.front.clone();
    front.size = size;
    state.save_tip(&app, tip.with_front(front), "size").await?;
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
    state.save_tip(&app, tip.with_front(front), "move").await?;
    Ok(state.snapshot())
}

/// Save the in-app edit form. `base` is the file stamp the form was opened
/// on: if the file changed on disk since, the edit is refused as a conflict
/// and the user's text stays in the form. The file name never changes.
#[tauri::command]
pub async fn update_tip(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    base: Option<String>,
    edit: TipEdit,
) -> Result<BoardState, String> {
    let tip = find(&state, &id)?;
    check_base(&tip, base.as_deref())?;
    let edited = edit::apply(&tip, edit)?;
    state.save_tip(&app, edited, "edit").await?;
    Ok(state.snapshot())
}

/// A file name for a new capture that is free both on the board and on disk
/// (a broken or not yet scanned file is in the folder but not on the board).
/// `also_taken` is a name that was just found to exist.
fn capture_name(
    state: &AppState,
    store: &FolderStore,
    stem: &str,
    also_taken: Option<&str>,
) -> String {
    state.with(|i| {
        i.board.unique_file_name_where(stem, |name| {
            also_taken.is_some_and(|t| t.eq_ignore_ascii_case(name))
                || store.resolve(name).map(|p| p.exists()).unwrap_or(false)
        })
    })
}

/// The form must have been opened on the version of the file the board has now.
fn check_base(tip: &Tip, base: Option<&str>) -> Result<(), String> {
    if tip.sha.as_deref() == base {
        Ok(())
    } else {
        Err(FolderError::Conflict(tip.path.clone()).to_string())
    }
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

/// Open the tip's file in the editor; returns the file path. Async and on a
/// blocking thread, so a slow or waiting editor launch never freezes the UI.
#[tauri::command]
pub async fn edit_local(app: AppHandle, id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = AppState::from_app(&app);
        editor::open(&state, &id).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| format!("editor launch failed: {e}"))?
}

/// Show the tip's file selected in Explorer.
#[tauri::command]
pub fn reveal_tip(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let tip = find(&state, &id)?;
    let path = state
        .store()?
        .resolve(&tip.path)
        .map_err(|e| e.to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(&path)
        .map_err(|e| format!("cannot show {}: {e}", path.display()))
}

/// Open the tips folder in Explorer.
#[tauri::command]
pub fn open_folder(state: State<'_, AppState>) -> Result<(), String> {
    let store = state.store()?;
    tauri_plugin_opener::open_path(store.root(), None::<&str>)
        .map_err(|e| format!("cannot open {}: {e}", store.root().display()))
}

/// Let the user pick a folder; `None` when the dialog is cancelled.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let dialog = app.dialog().file();
    // Owned by the board so it opens in front even when the board is on top.
    let dialog = match app.get_webview_window(windows::MAIN) {
        Some(main) => dialog.set_parent(&main),
        None => dialog,
    };
    let picked = dialog.blocking_pick_folder();
    picked
        .map(|p| p.into_path().map(|p| p.display().to_string()))
        .transpose()
        .map_err(|e| format!("cannot use that folder: {e}"))
}

/// Delete a tip's file and drop it from the board.
#[tauri::command]
pub async fn delete_tip(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<BoardState, String> {
    let tip = find(&state, &id)?;
    let stamp = tip
        .sha
        .clone()
        .ok_or("this tip has not been loaded from its file yet; try again in a moment")?;
    let store = state.store()?;
    log::info!("vindictive: delete {id}");
    let path = tip.path.clone();
    let deleted = tauri::async_runtime::spawn_blocking(move || {
        store.delete(&path, &stamp).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("delete failed: {e}"))?;
    match deleted {
        Ok(()) => {
            state.update_board(|board| board.remove(&id));
            state.emit(&app);
            Ok(state.snapshot())
        }
        Err(e) => {
            let _ = state.sync(&app).await;
            Err(e)
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
    let zh = state.settings().language == Language::Zh;
    let stem = file_stem(created_at, &front.title);
    let saved = save_new(&app, &state, &stem, "capture", |path| {
        template::render(path, front.clone(), zh, true).map_err(|e| e.to_string())
    })
    .await?;
    spawn_enrich(&app, &saved);
    Ok(state.snapshot())
}

/// A new tip with an arXiv id or DOI fetches its paper in the background.
fn spawn_enrich(app: &AppHandle, tip: &Tip) {
    if tip.front.arxiv.is_none() && tip.front.doi.is_none() {
        return;
    }
    let app = app.clone();
    let id = tip.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = enrich(&app, &id).await {
            log::warn!("auto enrich failed for {id}: {e}");
        }
    });
}

/// Create a tip from the full form. A body left blank gets a short skeleton
/// for the kind; the commented key reference is not written, since the form
/// already shows every key.
#[tauri::command]
pub async fn create_tip_full(
    app: AppHandle,
    state: State<'_, AppState>,
    edit: TipEdit,
) -> Result<BoardState, String> {
    let zh = state.settings().language == Language::Zh;
    let created_at = now();
    let (front, body) = form_tip(edit, created_at, zh)?;
    let stem = file_stem(created_at, &front.title);
    let saved = save_new(&app, &state, &stem, "create", |path| {
        template::render_with_body(path, front.clone(), &body, zh, false).map_err(|e| e.to_string())
    })
    .await?;
    spawn_enrich(&app, &saved);
    Ok(state.snapshot())
}

/// The frontmatter and body of a tip made with the form at `created_at`:
/// the form validated as an edit of a blank tip, the kind's skeleton when
/// the body was left blank, and a paper id found in the title or links.
fn form_tip(
    edit: TipEdit,
    created_at: NaiveDateTime,
    zh: bool,
) -> Result<(FrontMatter, String), String> {
    let blank = Tip::from_parts(
        "new.md",
        None,
        FrontMatter {
            created: Some(format_dt(created_at)),
            ..Default::default()
        },
        String::new(),
    );
    let tip = edit::apply(&blank, edit)?;
    let body = if tip.body.trim().is_empty() {
        template::body_for_kind(tip.front.kind, zh)
    } else {
        tip.body
    };
    let mut front = tip.front;
    (front.arxiv, front.doi) = paper_ids(&front.title, &front.links);
    Ok((front, body))
}

/// The paper a new tip names, as quick capture finds it: an arXiv id or DOI
/// among the title's words, else an arxiv.org or doi.org link. The first
/// arXiv id wins over any DOI, as `enrich` prefers arXiv too. The title is
/// left as typed.
fn paper_ids(title: &str, links: &[String]) -> (Option<String>, Option<String>) {
    let words: Vec<&str> = title.split_whitespace().collect();
    let doi_link = |l: &&str| {
        let lower = l.to_ascii_lowercase();
        lower.contains("doi.org/") || lower.starts_with("doi:") || lower.starts_with("10.")
    };
    let candidates = words
        .iter()
        .copied()
        .map(|w| (w, true))
        .chain(links.iter().map(|l| (l.trim(), false)));
    let mut doi = None;
    for (text, in_title) in candidates {
        if let Some(id) = capture::arxiv_id(text) {
            return (Some(id), None);
        }
        if doi.is_none() && (in_title || doi_link(&text)) {
            doi = capture::doi(text);
        }
    }
    (None, doi)
}

/// `2026-10-01-camera-ready`: the creation date and the title's slug.
fn file_stem(created_at: NaiveDateTime, title: &str) -> String {
    format!("{}-{}", created_at.format("%Y-%m-%d"), slugify(title))
}

/// Write a new tip under a free name built from `stem`. `render` gives the
/// tip and file text for a path. If another device created the same name
/// just now, the next free name is taken once.
async fn save_new(
    app: &AppHandle,
    state: &AppState,
    stem: &str,
    action: &str,
    render: impl Fn(&str) -> Result<(Tip, String), String>,
) -> Result<Tip, String> {
    let store = state.store()?;
    let path = capture_name(state, &store, stem, None);
    let (tip, text) = render(&path)?;
    match state.save_tip_text(app, tip, &text, action).await {
        Err(e) if e == FolderError::Exists(path.clone()).to_string() => {
            let path = capture_name(state, &store, stem, Some(&path));
            let (tip, text) = render(&path)?;
            state.save_tip_text(app, tip, &text, action).await
        }
        other => other,
    }
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<BoardState, String> {
    let clean = settings.validated()?;
    let previous = state.settings();
    let folder_changed = previous.folder != clean.folder;
    if folder_changed && clean.is_configured() {
        ensure_folder(Path::new(&clean.folder))?;
    }
    // Hotkeys before the save: one another app owns rejects the whole save
    // and the old hotkeys stay registered.
    hotkeys::switch(&app, &previous, &clean)?;
    // Save first: if that fails the board and the old folder stay as they were.
    if let Err(e) = state.set_settings(clean.clone()) {
        if let Err(back) = hotkeys::switch(&app, &clean, &previous) {
            log::warn!("restoring the old hotkeys: {back}");
        }
        return Err(e);
    }
    if folder_changed {
        state.with(|i| i.broken.clear());
        state.replace_board(Default::default());
    }
    windows::apply_settings(&app, &previous, &clean);
    state.emit(&app);
    if folder_changed && clean.is_configured() {
        let _ = state.sync(&app).await;
    }
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn fetch_image(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let path = path.trim();
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("data:") {
        return Ok(path.to_string());
    }
    let repo_path = image_path(path)?;
    if let Some(cached) = state.cached_image(&repo_path) {
        return Ok(cached);
    }
    let store = state.store()?;
    let rel = repo_path.clone();
    let data_url = tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_capped(&store, &rel, MAX_IMAGE_BYTES)?;
        Ok::<_, String>(format!(
            "data:{};base64,{}",
            mime_for(&rel),
            B64.encode(&bytes)
        ))
    })
    .await
    .map_err(|e| format!("image read failed: {e}"))??;
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
    let fetched = weather::fetch(&place, now)
        .await
        .map_err(|e| e.to_string())?;
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
    state.tip(id).ok_or_else(|| format!("tip {id} not found"))
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
    state
        .save_tip(app, enriched, "paper metadata")
        .await
        .map(|_| ())
}

/// The bytes of `rel`, refusing files over `max` before reading them.
fn read_capped(store: &FolderStore, rel: &str, max: usize) -> Result<Vec<u8>, String> {
    let too_big = || format!("image is larger than {} MB", max / (1024 * 1024));
    let path = store.resolve(rel).map_err(|e| e.to_string())?;
    let len = std::fs::metadata(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?
        .len();
    if len > max as u64 {
        return Err(too_big());
    }
    let bytes = store.read_bytes(rel).map_err(|e| e.to_string())?;
    // The file may have grown between the size check and the read.
    if bytes.len() > max {
        return Err(too_big());
    }
    Ok(bytes)
}

/// An image reference relative to the tips folder, with `/` separators.
/// Escaping the folder is refused later by `FolderStore::resolve`.
fn image_path(path: &str) -> Result<String, String> {
    let cleaned = path.trim().trim_start_matches('/').replace('\\', "/");
    if cleaned.is_empty() {
        return Err("empty image path".into());
    }
    Ok(cleaned)
}

/// Create the tips folder if needed, so picking a new folder just works.
pub fn ensure_folder(folder: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("cannot create tips folder {}: {e}", folder.display()))?;
    Ok(folder.to_path_buf())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_paths_are_folder_relative() {
        assert_eq!(image_path("figures/a.png").unwrap(), "figures/a.png");
        assert_eq!(image_path("/figures\\a.png").unwrap(), "figures/a.png");
        assert!(image_path("  ").is_err());
    }

    #[test]
    fn ensure_folder_creates_nested_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("OneDrive").join("Vindictive");
        ensure_folder(&target).unwrap();
        assert!(target.is_dir());
        ensure_folder(&target).unwrap();
    }

    #[test]
    fn form_tips_find_paper_ids_like_capture() {
        let at = chrono::NaiveDate::from_ymd_opt(2026, 11, 4)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let ids = |title: &str, links: &[&str]| {
            let (front, _) = form_tip(
                TipEdit {
                    title: title.into(),
                    links: links.iter().map(|l| l.to_string()).collect(),
                    ..Default::default()
                },
                at,
                false,
            )
            .unwrap();
            assert_eq!(front.title, title.trim(), "the title is kept as typed");
            (front.arxiv, front.doi)
        };
        let some = |s: &str| Some(s.to_string());
        assert_eq!(
            ids("Read 2401.12345 tonight", &[]),
            (some("2401.12345"), None)
        );
        assert_eq!(
            ids("Dowell paper 10.1049/piee.1966.0236", &[]),
            (None, some("10.1049/piee.1966.0236"))
        );
        assert_eq!(
            ids("Attention", &["https://arxiv.org/abs/1706.03762v7"]),
            (some("1706.03762"), None)
        );
        assert_eq!(
            ids("Loss model", &["https://doi.org/10.1109/TIE.2024.1234567"]),
            (None, some("10.1109/TIE.2024.1234567"))
        );
        // arXiv wins over a DOI, wherever it is.
        assert_eq!(
            ids("Paper 10.1/x", &["https://arxiv.org/pdf/2401.00001.pdf"]),
            (some("2401.00001"), None)
        );
        // Other links that merely contain "10.x/" are not DOIs.
        assert_eq!(
            ids("Plain task", &["https://example.org/10.1234/page"]),
            (None, None)
        );
        assert_eq!(ids("Buy milk", &[]), (None, None));
    }

    #[test]
    fn form_tips_get_a_dated_name_and_a_body() {
        let at = chrono::NaiveDate::from_ymd_opt(2026, 11, 4)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap();
        let form = TipEdit {
            title: "  Camera-ready v2 ".into(),
            kind: Kind::Deadline,
            due: Some("2026-11-20".into()),
            remind: vec!["-1d".into()],
            highlight: vec!["camera".into()],
            ..Default::default()
        };
        let (front, body) = form_tip(form.clone(), at, false).unwrap();
        assert_eq!(front.title, "Camera-ready v2");
        assert_eq!(front.created.as_deref(), Some("2026-11-04 14:30"));
        assert_eq!(front.remind, vec!["-1d"]);
        assert_eq!(front.highlight, vec!["camera"]);
        assert_eq!(body, template::body_for_kind(Kind::Deadline, false));
        assert_eq!(file_stem(at, &front.title), "2026-11-04-camera-ready-v2");
        // Chinese skeleton, and the user's own text wins over the skeleton.
        let (_, zh_body) = form_tip(form.clone(), at, true).unwrap();
        assert_eq!(zh_body, template::body_for_kind(Kind::Deadline, true));
        let (_, own) = form_tip(
            TipEdit {
                body: "My notes\r\nline 2".into(),
                ..form.clone()
            },
            at,
            false,
        )
        .unwrap();
        assert_eq!(own, "My notes\nline 2");
        let (_, note) = form_tip(
            TipEdit {
                kind: Kind::Note,
                body: "  \n ".into(),
                ..form.clone()
            },
            at,
            false,
        )
        .unwrap();
        assert_eq!(note, "");
        // The form is validated like an edit.
        assert!(form_tip(
            TipEdit {
                title: " ".into(),
                ..form.clone()
            },
            at,
            false
        )
        .is_err());
        assert!(form_tip(TipEdit { due: None, ..form }, at, false)
            .unwrap_err()
            .contains("due date"));
        // What gets written parses back to the same frontmatter, with no hints.
        let (front, body) = form_tip(
            TipEdit {
                title: "Read Dowell".into(),
                kind: Kind::Reading,
                ..Default::default()
            },
            at,
            false,
        )
        .unwrap();
        let (tip, text) = template::render_with_body(
            "2026-11-04-read-dowell.md",
            front.clone(),
            &body,
            false,
            false,
        )
        .unwrap();
        assert_eq!(tip.front, front);
        assert!(!text.contains("# ----"), "{text}");
        assert!(text.contains("## Why read it"), "{text}");
    }

    #[test]
    fn edits_must_start_from_the_current_file() {
        let tip = Tip::parse(
            "tips/a.md",
            Some("v2".into()),
            "---
title: A
---
",
        )
        .unwrap();
        assert!(check_base(&tip, Some("v2")).is_ok());
        let err = check_base(&tip, Some("v1")).unwrap_err();
        assert!(err.contains("changed on disk"), "{err}");
        assert!(check_base(&tip, None).is_err());
    }

    #[test]
    fn large_images_are_refused_before_reading() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("small.png"), [1u8; 4]).unwrap();
        std::fs::write(tmp.path().join("big.png"), [1u8; 5]).unwrap();
        let store = FolderStore::open(tmp.path()).unwrap();
        assert_eq!(read_capped(&store, "small.png", 4).unwrap().len(), 4);
        assert!(read_capped(&store, "big.png", 4).is_err());
        assert!(read_capped(&store, "missing.png", 4).is_err());
    }

    #[test]
    fn capture_names_skip_files_on_disk_and_known_clashes() {
        let folder = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let state = AppState::load(super::super::storage::Storage::new(
            data.path().to_path_buf(),
        ));
        let store = FolderStore::open(folder.path()).unwrap();
        assert_eq!(capture_name(&state, &store, "note", None), "note.md");
        // On disk but not on the board (say, it failed to parse).
        std::fs::write(folder.path().join("note.md"), "broken").unwrap();
        assert_eq!(capture_name(&state, &store, "note", None), "note-2.md");
        // A name the folder just reported as taken.
        assert_eq!(
            capture_name(&state, &store, "note", Some("note-2.md")),
            "note-3.md"
        );
    }

    #[test]
    fn mime_types() {
        assert_eq!(mime_for("x/y.PNG"), "image/png");
        assert_eq!(mime_for("a.jpeg"), "image/jpeg");
        assert_eq!(mime_for("noext"), "application/octet-stream");
    }
}
