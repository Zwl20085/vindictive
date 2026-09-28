//! User settings, persisted as JSON in the app config directory.

use serde::{Deserialize, Serialize};

use crate::sync::github::{validate_dir, validate_repo, RepoRef};
use crate::sync::weather::MAX_LOCATION_CHARS;

pub const MIN_POLL_SECONDS: u64 = 15;
pub const MAX_POLL_SECONDS: u64 = 3600;
pub const MIN_COLUMNS: u32 = 2;
pub const MAX_COLUMNS: u32 = 6;

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
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub dir: String,
    pub poll_seconds: u64,
    pub hotkey: String,
    pub dock: Dock,
    pub always_on_top: bool,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            owner: String::new(),
            repo: String::new(),
            branch: "main".into(),
            dir: "tips".into(),
            poll_seconds: 60,
            hotkey: "Ctrl+Shift+Space".into(),
            dock: Dock::Right,
            always_on_top: true,
            autostart: false,
            notify_new_tips: true,
            theme: Theme::Dark,
            columns: 4,
            show_done: false,
            language: Language::En,
            weather_location: String::new(),
            show_panel: true,
        }
    }
}

impl Settings {
    /// True once owner and repo are filled in.
    pub fn is_configured(&self) -> bool {
        !self.owner.trim().is_empty() && !self.repo.trim().is_empty()
    }

    pub fn repo_ref(&self) -> RepoRef {
        RepoRef {
            owner: self.owner.trim().to_string(),
            repo: self.repo.trim().to_string(),
            branch: self.branch.trim().to_string(),
        }
    }

    /// Validate and normalise. Returns a new, cleaned `Settings`.
    pub fn validated(&self) -> Result<Settings, String> {
        let mut s = self.clone();
        s.owner = s.owner.trim().to_string();
        s.repo = s.repo.trim().to_string();
        s.branch = s.branch.trim().to_string();
        if s.branch.is_empty() {
            s.branch = "main".into();
        }
        if s.is_configured() {
            validate_repo(&s.repo_ref()).map_err(|e| e.to_string())?;
        }
        s.dir = validate_dir(&s.dir).map_err(|e| e.to_string())?;
        if !(MIN_POLL_SECONDS..=MAX_POLL_SECONDS).contains(&s.poll_seconds) {
            return Err(format!(
                "poll interval must be between {MIN_POLL_SECONDS} and {MAX_POLL_SECONDS} seconds"
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
        Ok(s)
    }

    /// Repository path of the tips directory joined with a file name.
    pub fn tip_path(&self, file_name: &str) -> String {
        if self.dir.is_empty() {
            file_name.to_string()
        } else {
            format!("{}/{}", self.dir, file_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let s = Settings::default().validated().unwrap();
        assert_eq!(s.dir, "tips");
        assert!(!s.is_configured());
    }

    #[test]
    fn validation_catches_bad_values() {
        let s = Settings {
            poll_seconds: 5,
            ..Default::default()
        };
        assert!(s.validated().is_err());
        let s = Settings {
            columns: 9,
            ..Default::default()
        };
        assert!(s.validated().is_err());
        let s = Settings {
            dir: "../x".into(),
            ..Default::default()
        };
        assert!(s.validated().is_err());
        let s = Settings {
            owner: "a b".into(),
            repo: "r".into(),
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
    fn normalises() {
        let s = Settings {
            owner: " Zwl20085 ".into(),
            repo: "tips".into(),
            branch: "".into(),
            dir: "/notes/".into(),
            ..Default::default()
        }
        .validated()
        .unwrap();
        assert_eq!(s.owner, "Zwl20085");
        assert_eq!(s.branch, "main");
        assert_eq!(s.dir, "notes");
        assert_eq!(s.tip_path("a.md"), "notes/a.md");
        let root = Settings {
            dir: "".into(),
            ..Default::default()
        };
        assert_eq!(root.tip_path("a.md"), "a.md");
    }

    #[test]
    fn json_roundtrip_with_missing_keys() {
        let s: Settings = serde_json::from_str(r#"{"owner":"x"}"#).unwrap();
        assert_eq!(s.owner, "x");
        assert_eq!(s.poll_seconds, 60);
        assert_eq!(s.language, Language::En);
        assert!(s.show_panel);
        assert!(s.weather_location.is_empty());
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), s);
    }

    #[test]
    fn language_and_weather_fields() {
        let s: Settings =
            serde_json::from_str(r#"{"language":"zh","weather_location":"  Brisbane  "}"#)
                .unwrap();
        assert_eq!(s.language, Language::Zh);
        let clean = s.validated().unwrap();
        assert_eq!(clean.weather_location, "Brisbane");
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
