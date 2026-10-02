//! The two global shortcuts: quick capture (`hotkey`) and peek at the
//! board (`board_hotkey`, optional). One plugin handler serves both and
//! dispatches on the shortcut that fired.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use super::settings::Settings;
use super::state::AppState;
use super::{peek, windows};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Capture,
    Peek,
}

/// A configured shortcut; blank or unparsable means none.
fn parsed(text: &str) -> Option<Shortcut> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.parse().ok()).flatten()
}

/// What a fired shortcut means under `settings`.
pub fn action_for(fired: &Shortcut, settings: &Settings) -> Option<HotkeyAction> {
    if parsed(&settings.hotkey).as_ref() == Some(fired) {
        Some(HotkeyAction::Capture)
    } else if parsed(&settings.board_hotkey).as_ref() == Some(fired) {
        Some(HotkeyAction::Peek)
    } else {
        None
    }
}

/// The global-shortcut plugin's handler (key down only).
pub fn on_shortcut(app: &AppHandle, fired: &Shortcut) {
    let settings = AppState::from_app(app).settings();
    let result = match action_for(fired, &settings) {
        Some(HotkeyAction::Capture) => windows::show_capture(app),
        Some(HotkeyAction::Peek) => peek::toggle(app, &settings),
        None => Ok(()),
    };
    if let Err(e) = result {
        log::warn!("hotkey: {e}");
    }
}

/// Register both shortcuts afresh. Each is registered on its own, so one
/// that fails (taken by another app, say) never costs the other.
pub fn register_all(app: &AppHandle, settings: &Settings) {
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        log::warn!("could not clear the old hotkeys: {e}");
    }
    let wanted = [
        ("capture hotkey", settings.hotkey.as_str()),
        ("board hotkey", settings.board_hotkey.as_str()),
    ];
    for (what, text) in wanted {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let result = text
            .parse::<Shortcut>()
            .map_err(|e| format!("bad {what} {text:?}: {e}"))
            .and_then(|s| {
                gs.register(s)
                    .map_err(|e| format!("could not register {what} {text}: {e}"))
            });
        match result {
            Ok(()) => log::info!("{what}: {text}"),
            Err(e) => log::warn!("{e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> Shortcut {
        text.parse().unwrap()
    }

    #[test]
    fn dispatches_by_the_shortcut_that_fired() {
        let s = Settings::default();
        assert_eq!(
            action_for(&key("Ctrl+Shift+Space"), &s),
            Some(HotkeyAction::Capture)
        );
        assert_eq!(
            action_for(&key("Ctrl+Alt+Shift+Space"), &s),
            Some(HotkeyAction::Peek)
        );
        assert_eq!(action_for(&key("Ctrl+Alt+K"), &s), None);
    }

    #[test]
    fn a_blank_board_hotkey_peeks_at_nothing() {
        let s = Settings {
            board_hotkey: String::new(),
            ..Default::default()
        };
        assert_eq!(action_for(&key("Ctrl+Alt+Shift+Space"), &s), None);
        assert_eq!(
            action_for(&key("Ctrl+Shift+Space"), &s),
            Some(HotkeyAction::Capture)
        );
    }
}
