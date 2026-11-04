//! Window placement, the capture bar, window layers and autostart.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize,
    WebviewWindow,
};
use tauri_plugin_autostart::ManagerExt as _;

use super::hotkeys;
use super::peek;
use super::settings::{Dock, Settings};
use super::state::AppState;

pub const MAIN: &str = "main";
pub const CAPTURE: &str = "capture";
pub const EVENT_CAPTURE_SHOWN: &str = "capture-shown";
pub const EVENT_OPEN_SETTINGS: &str = "open-settings";
/// Sent to the board just before every size or position change we make, so
/// the front end can tell the `resize` that follows from one by the user.
pub const EVENT_APP_SIZED: &str = "app-sized";

/// Payload of `app-sized`: the board's inner size after the change, in
/// logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AppSized {
    pub width: f64,
    pub height: f64,
}

/// Gap between the board and the screen edge, in physical pixels.
const EDGE_MARGIN: i32 = 12;
/// Smallest height the board will shrink to when fitting its tiles.
const MIN_FIT_HEIGHT: f64 = 200.0;
/// Narrowest the board is ever sized at launch (matches `minWidth`).
const MIN_WIDTH: i32 = 160;
/// Showing the capture bar and focusing it makes Windows report a brief
/// loss of focus first; a blur this soon after showing is that hand-over.
const CAPTURE_SHOW_GRACE: Duration = Duration::from_millis(300);
/// When the capture bar was last shown.
static CAPTURE_SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);

fn window(app: &AppHandle, label: &str) -> Result<WebviewWindow, String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window {label} missing"))
}

pub fn show_main(app: &AppHandle) -> Result<(), String> {
    let w = window(app, MAIN)?;
    w.show()
        .and_then(|_| w.set_focus())
        .map_err(|e| e.to_string())
}

/// Hide the board, putting a lifted one back on its layer first so it never
/// comes back pinned on top.
pub fn hide_main(app: &AppHandle) -> Result<(), String> {
    let w = window(app, MAIN)?;
    peek::drop_back(app, &AppState::from_app(app).settings());
    peek::forget();
    w.hide().map_err(|e| e.to_string())
}

pub fn toggle_main(app: &AppHandle) -> Result<(), String> {
    let w = window(app, MAIN)?;
    if w.is_visible().map_err(|e| e.to_string())? {
        hide_main(app)
    } else {
        peek::reveal(app)
    }
}

pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    peek::reveal(app)?;
    app.emit_to(MAIN, EVENT_OPEN_SETTINGS, ())
        .map_err(|e| e.to_string())
}

/// Show the capture bar centred on the monitor under the cursor. If it is
/// already up, only focus it: the bar keeps its draft and is not reset.
pub fn show_capture(app: &AppHandle) -> Result<(), String> {
    let w = window(app, CAPTURE)?;
    if w.is_visible().map_err(|e| e.to_string())? {
        return w.set_focus().map_err(|e| e.to_string());
    }
    if let Err(e) = centre_capture(app, &w) {
        log::warn!("capture: centring on the cursor's monitor failed: {e}");
        w.center().map_err(|e| e.to_string())?;
    }
    *CAPTURE_SHOWN_AT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    app.emit_to(CAPTURE, EVENT_CAPTURE_SHOWN, ())
        .map_err(|e| e.to_string())
}

fn centre_capture(app: &AppHandle, w: &WebviewWindow) -> Result<(), String> {
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|e| e.to_string())?
        .or_else(|| w.primary_monitor().ok().flatten())
        .ok_or("no monitor found")?;
    let area = monitor.work_area();
    let size = w.outer_size().map_err(|e| e.to_string())?;
    let (x, y) = centre_position(
        (area.position.x, area.position.y),
        (area.size.width as i32, area.size.height as i32),
        (size.width as i32, size.height as i32),
    );
    w.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// True when a blur of the capture bar means the user left it, rather than
/// the focus hand-over right after it was shown.
pub fn capture_blur_hides(since_show: Option<Duration>) -> bool {
    !since_show.is_some_and(|d| d < CAPTURE_SHOW_GRACE)
}

/// The capture bar lost focus: hide it, unless this is the hand-over just
/// after showing; then look again once the grace is over.
pub fn on_capture_blur(app: &AppHandle) {
    let since_show = CAPTURE_SHOWN_AT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .map(|at| at.elapsed());
    if capture_blur_hides(since_show) {
        let _ = hide_capture(app);
        return;
    }
    let wait = CAPTURE_SHOW_GRACE.saturating_sub(since_show.unwrap_or_default());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(wait).await;
        if let Ok(w) = window(&app, CAPTURE) {
            if !w.is_focused().unwrap_or(false) {
                let _ = w.hide();
            }
        }
    });
}

pub fn hide_capture(app: &AppHandle) -> Result<(), String> {
    window(app, CAPTURE)?.hide().map_err(|e| e.to_string())
}

/// Size the board to half the work area width and its full height (minus
/// the edge margin). Runs on every launch so the board always opens at
/// half the desktop.
pub fn size_main_half(app: &AppHandle) -> Result<(), String> {
    let w = window(app, MAIN)?;
    let monitor = monitor_of(&w)?;
    let area = monitor.work_area();
    let (width, height) = half_screen_size((area.size.width as i32, area.size.height as i32));
    let size = PhysicalSize::new(width, height);
    let scale = w.scale_factor().map_err(|e| e.to_string())?;
    announce_main(app, Some(size.to_logical(scale)));
    w.set_size(size).map_err(|e| e.to_string())
}

/// Every programmatic size or position change of the board goes through
/// here first: emit `app-sized` with the inner size the board will have
/// (`next`, or its current size for a move) in logical pixels.
fn announce_main(app: &AppHandle, next: Option<LogicalSize<f64>>) {
    let size = match next {
        Some(size) => Ok(size),
        None => window(app, MAIN).and_then(|w| {
            let scale = w.scale_factor().map_err(|e| e.to_string())?;
            Ok(w.inner_size()
                .map_err(|e| e.to_string())?
                .to_logical::<f64>(scale))
        }),
    };
    let result = size.and_then(|s| {
        app.emit_to(
            MAIN,
            EVENT_APP_SIZED,
            AppSized {
                width: s.width,
                height: s.height,
            },
        )
        .map_err(|e| e.to_string())
    });
    if let Err(e) = result {
        log::warn!("app-sized: {e}");
    }
}

/// Half the work area width and the full height, both minus the edge
/// margins, in physical pixels. Never smaller than the window minimum.
pub fn half_screen_size(area: (i32, i32)) -> (u32, u32) {
    let width = (area.0 / 2 - 2 * EDGE_MARGIN).max(MIN_WIDTH);
    let height = (area.1 - 2 * EDGE_MARGIN).max(MIN_FIT_HEIGHT as i32);
    (width as u32, height as u32)
}

fn monitor_of(w: &WebviewWindow) -> Result<Monitor, String> {
    w.current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| w.primary_monitor().ok().flatten())
        .ok_or_else(|| "no monitor found".to_string())
}

/// Move the board to a screen edge, vertically centred in the work area.
/// A free board stays where the user put it, only pulled back inside the
/// work area if its size (startup sizing, a fit) pushed it past an edge.
pub fn dock_main(app: &AppHandle, dock: Dock) -> Result<(), String> {
    let w = window(app, MAIN)?;
    let monitor = monitor_of(&w)?;
    let area = monitor.work_area();
    let origin = (area.position.x, area.position.y);
    let area = (area.size.width as i32, area.size.height as i32);
    let size = w.outer_size().map_err(|e| e.to_string())?;
    let win = (size.width as i32, size.height as i32);
    let (x, y) = if dock == Dock::Free {
        let at = w.outer_position().map_err(|e| e.to_string())?;
        let clamped = clamp_position(origin, area, win, (at.x, at.y));
        if clamped == (at.x, at.y) {
            return Ok(());
        }
        clamped
    } else {
        dock_position(dock, origin, area, win)
    };
    announce_main(app, None);
    w.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// Pull `pos` back so a `win`-sized window lies inside the work area; a
/// window larger than the area is pinned to its top-left corner.
pub fn clamp_position(
    origin: (i32, i32),
    area: (i32, i32),
    win: (i32, i32),
    pos: (i32, i32),
) -> (i32, i32) {
    let axis = |start: i32, len: i32, size: i32, at: i32| at.min(start + len - size).max(start);
    (
        axis(origin.0, area.0, win.0, pos.0),
        axis(origin.1, area.1, win.1, pos.1),
    )
}

/// Top-left corner that centres a `win`-sized window in the work area.
pub fn centre_position(origin: (i32, i32), area: (i32, i32), win: (i32, i32)) -> (i32, i32) {
    (
        origin.0 + ((area.0 - win.0) / 2).max(0),
        origin.1 + ((area.1 - win.1) / 2).max(0),
    )
}

/// Resize the board to `height` logical pixels (clamped to the work area
/// minus the edge margin), then re-dock so it stays on its edge.
pub fn fit_main(app: &AppHandle, dock: Dock, height: f64) -> Result<(), String> {
    let w = window(app, MAIN)?;
    let monitor = monitor_of(&w)?;
    let scale = w.scale_factor().map_err(|e| e.to_string())?;
    let area_h = monitor.work_area().size.height as f64 / scale - 2.0 * EDGE_MARGIN as f64 / scale;
    let target = fit_height(height, area_h);
    let current = w
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    if (current.height - target).abs() < 1.0 {
        return Ok(());
    }
    let next = LogicalSize::new(current.width, target);
    announce_main(app, Some(next));
    w.set_size(next).map_err(|e| e.to_string())?;
    dock_main(app, dock)
}

/// Clamp a wanted height into `[MIN_FIT_HEIGHT, work area]`.
pub fn fit_height(wanted: f64, area: f64) -> f64 {
    wanted
        .max(MIN_FIT_HEIGHT)
        .min(area.max(MIN_FIT_HEIGHT))
        .round()
}

/// Pure placement maths, testable without a display.
pub fn dock_position(
    dock: Dock,
    origin: (i32, i32),
    area: (i32, i32),
    win: (i32, i32),
) -> (i32, i32) {
    let y = origin.1 + ((area.1 - win.1) / 2).max(0);
    let x = match dock {
        Dock::Left => origin.0 + EDGE_MARGIN,
        Dock::Right | Dock::Free => origin.0 + area.0 - win.0 - EDGE_MARGIN,
    };
    (x, y)
}

pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(e) = result {
        log::warn!("autostart change failed: {e}");
    }
}

/// One window-layer flag: its log name and the setter that applies it.
type LayerStep = (&'static str, fn(&WebviewWindow, bool) -> tauri::Result<()>);
const TOP: LayerStep = ("always on top", WebviewWindow::set_always_on_top);
const BOTTOM: LayerStep = ("always on bottom", WebviewWindow::set_always_on_bottom);

/// Pin the board above or below every other window, or neither. The flag
/// being switched off is cleared first so the OS never sees both at once.
pub fn apply_layer(w: &WebviewWindow, top: bool, bottom: bool) {
    let order = if top {
        [(BOTTOM, false), (TOP, true)]
    } else {
        [(TOP, false), (BOTTOM, bottom)]
    };
    for ((name, set), on) in order {
        if let Err(e) = set(w, on) {
            log::warn!("{name} failed: {e}");
        }
    }
}

/// React to a settings change. Errors are logged, never fatal. Hotkeys are
/// not touched here: `hotkeys::switch` runs before the save, so that a
/// combination another app owns rejects it.
pub fn apply_settings(app: &AppHandle, previous: &Settings, next: &Settings) {
    if previous.autostart != next.autostart {
        apply_autostart(app, next.autostart);
    }
    if previous.always_on_top != next.always_on_top
        || previous.always_on_bottom != next.always_on_bottom
    {
        peek::forget();
        if let Ok(w) = window(app, MAIN) {
            apply_layer(&w, next.always_on_top, next.always_on_bottom);
        }
    }
    if previous.dock != next.dock {
        if let Err(e) = dock_main(app, next.dock) {
            log::warn!("dock failed: {e}");
        }
    }
}

/// Apply everything at startup.
pub fn apply_initial(app: &AppHandle, settings: &Settings) {
    if let Err(e) = hotkeys::register_all(app, settings) {
        log::warn!("{e}");
    }
    apply_autostart(app, settings.autostart);
    if let Ok(w) = window(app, MAIN) {
        apply_layer(&w, settings.always_on_top, settings.always_on_bottom);
    }
    if let Err(e) = size_main_half(app) {
        log::warn!("half-screen size failed: {e}");
    }
    if let Err(e) = dock_main(app, settings.dock) {
        log::warn!("dock failed: {e}");
    }
    // The window is created hidden so the placeholder size never flashes.
    if let Err(e) = show_main(app) {
        log::warn!("show failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_height_clamps() {
        assert_eq!(fit_height(500.0, 1000.0), 500.0);
        assert_eq!(fit_height(50.0, 1000.0), MIN_FIT_HEIGHT);
        assert_eq!(fit_height(5000.0, 1000.0), 1000.0);
        assert_eq!(fit_height(5000.0, 100.0), MIN_FIT_HEIGHT);
    }

    #[test]
    fn half_screen_sizes() {
        assert_eq!(
            half_screen_size((1920, 1040)),
            (960 - 2 * EDGE_MARGIN as u32, 1040 - 2 * EDGE_MARGIN as u32)
        );
        assert_eq!(
            half_screen_size((2560, 1400)),
            (1280 - 2 * EDGE_MARGIN as u32, 1400 - 2 * EDGE_MARGIN as u32)
        );
        // Tiny work areas never go below the window minimum.
        assert_eq!(
            half_screen_size((200, 100)),
            (MIN_WIDTH as u32, MIN_FIT_HEIGHT as u32)
        );
    }

    #[test]
    fn free_boards_are_pulled_back_inside_the_work_area() {
        let origin = (0, 0);
        let area = (1920, 1040);
        let win = (400, 1016);
        // Inside already: unchanged.
        assert_eq!(clamp_position(origin, area, win, (100, 12)), (100, 12));
        // Grown past the bottom edge: moved up just enough.
        assert_eq!(clamp_position(origin, area, win, (100, 300)), (100, 24));
        // Past the left and right edges.
        assert_eq!(clamp_position(origin, area, win, (-50, 0)), (0, 0));
        assert_eq!(clamp_position(origin, area, win, (1800, 0)), (1520, 0));
        // Larger than the area: top-left corner of the area.
        assert_eq!(
            clamp_position((-2560, 100), (800, 600), (900, 700), (-2000, 500)),
            (-2560, 100)
        );
    }

    #[test]
    fn capture_bar_centres_in_the_work_area() {
        assert_eq!(centre_position((0, 0), (1920, 1040), (480, 96)), (720, 472));
        assert_eq!(
            centre_position((-2560, 100), (2560, 1400), (480, 96)),
            (-2560 + 1040, 100 + 652)
        );
        assert_eq!(centre_position((0, 0), (100, 50), (480, 96)), (0, 0));
    }

    #[test]
    fn the_capture_hand_over_blur_does_not_hide_it() {
        assert!(!capture_blur_hides(Some(Duration::from_millis(30))));
        assert!(capture_blur_hides(Some(Duration::from_secs(2))));
        assert!(capture_blur_hides(None));
    }

    #[test]
    fn app_sized_payload_matches_the_front_end_type() {
        let json = serde_json::to_value(AppSized {
            width: 290.0,
            height: 620.5,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({ "width": 290.0, "height": 620.5 }));
    }

    #[test]
    fn dock_positions() {
        let origin = (0, 0);
        let area = (1920, 1040);
        let win = (290, 620);
        assert_eq!(
            dock_position(Dock::Right, origin, area, win),
            (1920 - 290 - EDGE_MARGIN, 210)
        );
        assert_eq!(
            dock_position(Dock::Left, origin, area, win),
            (EDGE_MARGIN, 210)
        );
        // Second monitor to the left of the primary.
        assert_eq!(
            dock_position(Dock::Left, (-2560, 100), (2560, 1400), win),
            (-2560 + EDGE_MARGIN, 490)
        );
        // Window taller than the work area never goes negative.
        assert_eq!(dock_position(Dock::Right, origin, (800, 500), win).1, 0);
    }
}
