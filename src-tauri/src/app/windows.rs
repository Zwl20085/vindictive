//! Window placement, the capture bar, the global hotkey and autostart.

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use super::settings::{Dock, Settings};

pub const MAIN: &str = "main";
pub const CAPTURE: &str = "capture";
pub const EVENT_CAPTURE_SHOWN: &str = "capture-shown";
pub const EVENT_OPEN_SETTINGS: &str = "open-settings";

/// Gap between the board and the screen edge, in physical pixels.
const EDGE_MARGIN: i32 = 12;
/// Smallest height the board will shrink to when fitting its tiles.
const MIN_FIT_HEIGHT: f64 = 200.0;
/// Narrowest the board is ever sized at launch (matches `minWidth`).
const MIN_WIDTH: i32 = 160;

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

pub fn toggle_main(app: &AppHandle) -> Result<(), String> {
    let w = window(app, MAIN)?;
    if w.is_visible().map_err(|e| e.to_string())? {
        w.hide().map_err(|e| e.to_string())
    } else {
        show_main(app)
    }
}

pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    show_main(app)?;
    app.emit_to(MAIN, EVENT_OPEN_SETTINGS, ())
        .map_err(|e| e.to_string())
}

pub fn show_capture(app: &AppHandle) -> Result<(), String> {
    let w = window(app, CAPTURE)?;
    w.center().map_err(|e| e.to_string())?;
    w.show().map_err(|e| e.to_string())?;
    w.set_focus().map_err(|e| e.to_string())?;
    app.emit_to(CAPTURE, EVENT_CAPTURE_SHOWN, ())
        .map_err(|e| e.to_string())
}

pub fn hide_capture(app: &AppHandle) -> Result<(), String> {
    window(app, CAPTURE)?.hide().map_err(|e| e.to_string())
}

/// Size the board to half the work area width and its full height (minus
/// the edge margin). Runs on every launch so the board always opens at
/// half the desktop.
pub fn size_main_half(app: &AppHandle) -> Result<(), String> {
    let w = window(app, MAIN)?;
    let monitor = w
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| w.primary_monitor().ok().flatten())
        .ok_or("no monitor found")?;
    let area = monitor.work_area();
    let (width, height) = half_screen_size((area.size.width as i32, area.size.height as i32));
    w.set_size(PhysicalSize::new(width, height))
        .map_err(|e| e.to_string())
}

/// Half the work area width and the full height, both minus the edge
/// margins, in physical pixels. Never smaller than the window minimum.
pub fn half_screen_size(area: (i32, i32)) -> (u32, u32) {
    let width = (area.0 / 2 - 2 * EDGE_MARGIN).max(MIN_WIDTH);
    let height = (area.1 - 2 * EDGE_MARGIN).max(MIN_FIT_HEIGHT as i32);
    (width as u32, height as u32)
}

/// Move the board to a screen edge, vertically centred in the work area.
pub fn dock_main(app: &AppHandle, dock: Dock) -> Result<(), String> {
    if dock == Dock::Free {
        return Ok(());
    }
    let w = window(app, MAIN)?;
    let monitor = w
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| w.primary_monitor().ok().flatten())
        .ok_or("no monitor found")?;
    let area = monitor.work_area();
    let size = w.outer_size().map_err(|e| e.to_string())?;
    let (x, y) = dock_position(
        dock,
        (area.position.x, area.position.y),
        (area.size.width as i32, area.size.height as i32),
        (size.width as i32, size.height as i32),
    );
    w.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// Resize the board to `height` logical pixels (clamped to the work area
/// minus the edge margin), then re-dock so it stays on its edge.
pub fn fit_main(app: &AppHandle, dock: Dock, height: f64) -> Result<(), String> {
    let w = window(app, MAIN)?;
    let monitor = w
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or_else(|| w.primary_monitor().ok().flatten())
        .ok_or("no monitor found")?;
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
    w.set_size(LogicalSize::new(current.width, target))
        .map_err(|e| e.to_string())?;
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

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|e| format!("bad hotkey {hotkey:?}: {e}"))?;
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    gs.register(shortcut)
        .map_err(|e| format!("could not register {hotkey}: {e}"))
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

/// React to a settings change. Errors are logged, never fatal.
pub fn apply_settings(app: &AppHandle, previous: &Settings, next: &Settings) {
    if previous.hotkey != next.hotkey {
        if let Err(e) = register_hotkey(app, &next.hotkey) {
            log::warn!("{e}");
        }
    }
    if previous.autostart != next.autostart {
        apply_autostart(app, next.autostart);
    }
    if previous.always_on_top != next.always_on_top {
        if let Ok(w) = window(app, MAIN) {
            if let Err(e) = w.set_always_on_top(next.always_on_top) {
                log::warn!("always on top failed: {e}");
            }
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
    if let Err(e) = register_hotkey(app, &settings.hotkey) {
        log::warn!("{e}");
    }
    apply_autostart(app, settings.autostart);
    if let Ok(w) = window(app, MAIN) {
        let _ = w.set_always_on_top(settings.always_on_top);
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
