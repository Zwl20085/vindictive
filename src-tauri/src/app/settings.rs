//! User settings, persisted as JSON in the app config directory.

use serde::{Deserialize, Serialize};

use std::path::Path;

use crate::sync::weather::MAX_LOCATION_CHARS;

pub const MIN_COLUMNS: u32 = 2;
pub const MAX_COLUMNS: u32 = 6;
/// Window opacity range, percent. Below the minimum the board is unusable.
pub const MIN_WINDOW_OPACITY: u8 = 20;
pub const MAX_WINDOW_OPACITY: u8 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    Left,
    #[default]
    Right,
    Free,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Nerv,
    Cobalt,
    Paper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Zh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Absolute path of the folder holding the tip files, normally inside
    /// OneDrive. Empty until the first start fills in the default.
    pub folder: String,
    pub hotkey: String,
    pub dock: Dock,
    pub always_on_top: bool,
    /// Keep the board beneath every other window (desktop widget style).
    /// Mutually exclusive with `always_on_top`.
    pub always_on_bottom: bool,
    pub autostart: bool,
    pub notify_new_tips: bool,
    pub theme: Theme,
    pub columns: u32,
    pub show_done: bool,
    /// UI language of the board and capture windows.
    pub language: Language,
    /// Place name for the weather panel; empty turns weather off.
    pub weather_location: String,
    /// Show the clock / date / weather panel above the tiles.
    pub show_panel: bool,
    /// Command that opens a file for "Edit locally"; blank means the system
    /// default application for `.md`.
    pub editor_command: String,
    /// Resize the window height to fit the tiles (up to the work area).
    pub fit_height: bool,
    /// Background opacity of the whole window, percent (20..=100). Text stays solid.
    pub window_opacity: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            folder: String::new(),
            hotkey: "Ctrl+Shift+Space".into(),
            dock: Dock::Right,
            always_on_top: false,
            always_on_bottom: true,
            autostart: true,
            notify_new_tips: true,
            theme: Theme::Dark,
            columns: 4,
            show_done: false,
            language: Language::En,
            weather_location: "Nottingham".into(),
            show_panel: true,
            editor_command: "code".into(),
            fit_height: true,
            window_opacity: 50,
        }
    }
}

impl Settings {
    /// Settings written before "always on bottom" existed (or by hand) may
    /// ask for both layers; "on top" was the explicit choice, so it wins.
    pub fn repaired(self) -> Settings {
        if self.always_on_top && self.always_on_bottom {
            Settings {
                always_on_bottom: false,
                ..self
            }
        } else {
            self
        }
    }

    /// True once a tips folder is set.
    pub fn is_configured(&self) -> bool {
        !self.folder.trim().is_empty()
    }

    /// Validate and normalise. Returns a new, cleaned `Settings`.
    pub fn validated(&self) -> Result<Settings, String> {
        let mut s = self.clone();
        s.folder = s.folder.trim().to_string();
        if s.is_configured() && !Path::new(&s.folder).is_absolute() {
            return Err(format!(
                "tips folder must be a full path such as C:/Users/you/OneDrive/Vindictive, not {:?}",
                s.folder
            ));
        }
        if !(MIN_COLUMNS..=MAX_COLUMNS).contains(&s.columns) {
            return Err(format!(
                "columns must be between {MIN_COLUMNS} and {MAX_COLUMNS}"
            ));
        }
        s.hotkey = s.hotkey.trim().to_string();
        if s.hotkey.is_empty() {
            return Err("hotkey cannot be empty".into());
        }
        s.hotkey
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|e| format!("hotkey {:?} is not valid: {e}", s.hotkey))?;
        s.weather_location = s
            .weather_location
            .trim()
            .chars()
            .take(MAX_LOCATION_CHARS)
            .collect();
        s.editor_command = s.editor_command.trim().to_string();
        if !(MIN_WINDOW_OPACITY..=MAX_WINDOW_OPACITY).contains(&s.window_opacity) {
            return Err(format!(
                "window opacity must be between {MIN_WINDOW_OPACITY} and {MAX_WINDOW_OPACITY} percent"
            ));
        }
        if s.always_on_top && s.always_on_bottom {
            return Err("always on top and always on bottom cannot both be on".into());
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let s = Settings::default().validated().unwrap();
        assert!(s.folder.is_empty());
        assert!(!s.is_configured());
    }

    #[test]
    fn validation_catches_bad_values() {
        let s = Settings {
            columns: 9,
            ..Default::default()
        };
        assert!(s.validated().is_err());
        let s = Settings {
            folder: "relative/tips".into(),
            ..Default::default()
        };
        assert!(s.validated().is_err());
        let s = Settings {
            hotkey: "  ".into(),
            ..Default::default()
        };
        assert!(s.validated().is_err());
    }

    #[test]
    fn hotkey_must_parse() {
        let s = Settings {
            hotkey: "Ctrl+Shift+Space".into(),
            ..Default::default()
        };
        assert!(s.validated().is_ok());
        let s = Settings {
            hotkey: "Ctrl+Banana".into(),
            ..Default::default()
        };
        assert!(s.validated().is_err());
    }

    #[test]
    fn normalises_the_folder() {
        let s = Settings {
            folder: "  C:/Users/me/OneDrive/Vindictive  ".into(),
            ..Default::default()
        }
        .validated()
        .unwrap();
        assert_eq!(s.folder, "C:/Users/me/OneDrive/Vindictive");
        assert!(s.is_configured());
    }

    #[test]
    fn old_github_settings_still_load() {
        let s: Settings = serde_json::from_str(
            r#"{"owner":"x","repo":"y","branch":"main","dir":"tips","poll_seconds":60,"push_interval_minutes":60,"theme":"light"}"#,
        )
        .unwrap();
        assert_eq!(s.theme, Theme::Light);
        assert!(!s.is_configured());
    }

    #[test]
    fn json_roundtrip_with_missing_keys() {
        let s: Settings = serde_json::from_str(r#"{"folder":"D:/tips"}"#).unwrap();
        assert_eq!(s.folder, "D:/tips");
        assert_eq!(s.language, Language::En);
        assert!(s.show_panel);
        assert_eq!(s.editor_command, "code");
        assert!(s.fit_height);
        assert!(!s.always_on_top);
        assert!(s.always_on_bottom);
        assert!(s.autostart);
        assert_eq!(s.window_opacity, 50);
        let dim = Settings {
            window_opacity: 5,
            ..Default::default()
        };
        assert!(dim.validated().is_err());
        assert_eq!(s.weather_location, "Nottingham");
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
    }

    #[test]
    fn top_and_bottom_are_exclusive() {
        let both = Settings {
            always_on_top: true,
            always_on_bottom: true,
            ..Default::default()
        };
        assert!(both.validated().is_err());
        let bottom = Settings::default().validated().unwrap();
        assert!(bottom.always_on_bottom, "on bottom by default");
        assert!(!bottom.always_on_top);
        // An old file that only says "on top" gets the new default too, and
        // is repaired rather than left invalid.
        let old: Settings = serde_json::from_str(r#"{"always_on_top":true}"#).unwrap();
        let fixed = old.repaired();
        assert!(fixed.always_on_top && !fixed.always_on_bottom);
        assert!(fixed.validated().is_ok());
        let parsed: Settings = serde_json::from_str(r#"{"always_on_bottom":true}"#).unwrap();
        assert!(parsed.always_on_bottom);
    }

    #[test]
    fn language_and_weather_fields() {
        let s: Settings =
            serde_json::from_str(r#"{"language":"zh","weather_location":"  Tokyo  "}"#).unwrap();
        assert_eq!(s.language, Language::Zh);
        let clean = s.validated().unwrap();
        assert_eq!(clean.weather_location, "Tokyo");
        assert!(serde_json::to_string(&clean)
            .unwrap()
            .contains("\"language\":\"zh\""));
        assert!(serde_json::from_str::<Settings>(r#"{"language":"fr"}"#).is_err());
        let long = Settings {
            weather_location: "x".repeat(200),
            ..Default::default()
        }
        .validated()
        .unwrap();
        assert_eq!(long.weather_location.chars().count(), MAX_LOCATION_CHARS);
    }
}
