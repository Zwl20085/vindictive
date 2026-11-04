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
/// that fails (taken by another app, say) never costs the other; the
/// failures come back together, naming each hotkey.
pub fn register_all(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        log::warn!("could not clear the old hotkeys: {e}");
    }
    register_each(settings, |s| gs.register(s).map_err(|e| e.to_string()))
}

/// Register `settings`' hotkeys through `register`, collecting failures
/// into one message.
pub fn register_each(
    settings: &Settings,
    mut register: impl FnMut(Shortcut) -> Result<(), String>,
) -> Result<(), String> {
    let wanted = [
        ("capture hotkey", settings.hotkey.as_str()),
        ("peek hotkey", settings.board_hotkey.as_str()),
    ];
    let mut failures = Vec::new();
    for (what, text) in wanted {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let result = text
            .parse::<Shortcut>()
            .map_err(|e| format!("the {what} {text} is not a valid combination: {e}"))
            .and_then(|s| {
                register(s).map_err(|e| {
                    format!(
                        "the {what} {text} could not be registered (another app may own it): {e}"
                    )
                })
            });
        match result {
            Ok(()) => log::info!("{what}: {text}"),
            Err(e) => failures.push(e),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// Move from `previous`' hotkeys to `next`'s, before the settings are saved.
/// If any of the new ones cannot be registered, the old ones are put back
/// and the error names the hotkey, so the save can be rejected.
pub fn switch(app: &AppHandle, previous: &Settings, next: &Settings) -> Result<(), String> {
    if previous.hotkey == next.hotkey && previous.board_hotkey == next.board_hotkey {
        return Ok(());
    }
    register_all(app, next).inspect_err(|_| {
        if let Err(back) = register_all(app, previous) {
            log::warn!("restoring the old hotkeys: {back}");
        }
    })
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
    fn registration_failures_name_the_hotkey() {
        let s = Settings::default();
        let taken = key("Ctrl+Alt+Shift+Space");
        let err = register_each(&s, |k| {
            if k == taken {
                Err("already registered".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(err.contains("peek hotkey Ctrl+Alt+Shift+Space"), "{err}");
        assert!(!err.contains("capture hotkey"), "{err}");
    }

    #[test]
    fn both_hotkeys_are_tried_even_when_one_fails() {
        let s = Settings::default();
        let mut tried = 0;
        let err = register_each(&s, |_| {
            tried += 1;
            Err("taken".into())
        })
        .unwrap_err();
        assert_eq!(tried, 2);
        assert!(err.contains("capture hotkey") && err.contains("peek hotkey"));
    }

    #[test]
    fn a_blank_peek_hotkey_registers_only_capture() {
        let s = Settings {
            board_hotkey: String::new(),
            ..Default::default()
        };
        let mut tried = Vec::new();
        assert!(register_each(&s, |k| {
            tried.push(k);
            Ok(())
        })
        .is_ok());
        assert_eq!(tried, vec![key("Ctrl+Shift+Space")]);
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
