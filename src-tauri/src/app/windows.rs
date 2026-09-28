//! Window placement, the capture bar, the global hotkey and autostart.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use super::settings::{Dock, Settings};

pub const MAIN: &str = "main";
pub const CAPTURE: &str = "capture";
pub const EVENT_CAPTURE_SHOWN: &str = "capture-shown";
pub const EVENT_OPEN_SETTINGS: &str = "open-settings";

/// Gap between the board and the screen edge, in physical pixels.
const EDGE_MARGIN: i32 = 12;

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
    if let Err(e) = dock_main(app, settings.dock) {
        log::warn!("dock failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
