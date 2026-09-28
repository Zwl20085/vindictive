//! Small JSON files under the app data directory: settings, tip cache,
//! and the set of reminders already fired.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

use super::editor::EditSession;
use super::settings::Settings;
use crate::core::tip::Tip;

pub const SETTINGS_FILE: &str = "settings.json";
pub const CACHE_FILE: &str = "tips-cache.json";
pub const FIRED_FILE: &str = "fired.json";
pub const EDITS_FILE: &str = "edits.json";

#[derive(Debug, Clone)]
pub struct Storage {
    dir: PathBuf,
}

impl Storage {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn load_settings(&self) -> Settings {
        self.read(SETTINGS_FILE).unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> Result<(), String> {
        self.write(SETTINGS_FILE, s)
    }

    pub fn load_cache(&self) -> Vec<Tip> {
        self.read(CACHE_FILE).unwrap_or_default()
    }

    pub fn save_cache(&self, tips: &[Tip]) -> Result<(), String> {
        self.write(CACHE_FILE, &tips)
    }

    pub fn load_fired(&self) -> BTreeSet<String> {
        self.read(FIRED_FILE).unwrap_or_default()
    }

    pub fn save_fired(&self, fired: &BTreeSet<String>) -> Result<(), String> {
        self.write(FIRED_FILE, fired)
    }

    pub fn load_edits(&self) -> Vec<EditSession> {
        self.read(EDITS_FILE).unwrap_or_default()
    }

    pub fn save_edits(&self, edits: &[EditSession]) -> Result<(), String> {
        self.write(EDITS_FILE, &edits)
    }

    fn read<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let path = self.dir.join(name);
        let text = std::fs::read_to_string(&path).ok()?;
        match serde_json::from_str(&text) {
            Ok(v) => Some(v),
            Err(e) => {
                log::warn!("ignoring corrupt {}: {e}", path.display());
                None
            }
        }
    }

    fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot create data dir: {e}"))?;
        let path = self.dir.join(name);
        let tmp = self.dir.join(format!("{name}.tmp"));
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let st = Storage::new(tmp.path().join("nested"));
        assert_eq!(st.load_settings(), Settings::default());
        let s = Settings {
            owner: "me".into(),
            ..Default::default()
        };
        st.save_settings(&s).unwrap();
        assert_eq!(st.load_settings(), s);

        let tip = Tip::parse("tips/a.md", Some("s".into()), "---\ntitle: A\n---\nbody").unwrap();
        st.save_cache(std::slice::from_ref(&tip)).unwrap();
        assert_eq!(st.load_cache(), vec![tip]);

        let fired: BTreeSet<String> = ["a@1".to_string()].into_iter().collect();
        st.save_fired(&fired).unwrap();
        assert_eq!(st.load_fired(), fired);
    }

    #[test]
    fn corrupt_file_falls_back_to_default() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(SETTINGS_FILE), "{not json").unwrap();
        let st = Storage::new(tmp.path().to_path_buf());
        assert_eq!(st.load_settings(), Settings::default());
    }
}
