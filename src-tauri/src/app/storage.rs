//! Small JSON files under the app data directory: settings, tip cache,
//! and the set of reminders already fired.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use super::settings::Settings;
use crate::core::tip::Tip;

pub const SETTINGS_FILE: &str = "settings.json";
pub const CACHE_FILE: &str = "tips-cache.json";
pub const FIRED_FILE: &str = "fired.json";

/// Makes temp file names unique within this process.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const RENAME_RETRIES: u64 = 5;
const RENAME_BACKOFF_MS: u64 = 20;

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
        self.read::<Settings>(SETTINGS_FILE)
            .unwrap_or_default()
            .repaired()
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

    /// Write `value` as JSON to `name` atomically: a uniquely named temp
    /// file (so concurrent writers never share one) is flushed to disk and
    /// renamed over the target. The rename is retried briefly because on
    /// Windows a reader or virus scanner holding the target causes a
    /// transient sharing violation.
    fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot create data dir: {e}"))?;
        let path = self.dir.join(name);
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = self
            .dir
            .join(format!("{name}.{}-{n}.tmp", std::process::id()));
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        let written = (|| {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("cannot write {}: {e}", tmp.display()));
        }
        let mut attempt = 0;
        loop {
            match std::fs::rename(&tmp, &path) {
                Ok(()) => return Ok(()),
                Err(_) if attempt < RENAME_RETRIES => {
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(RENAME_BACKOFF_MS * attempt));
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(format!("cannot replace {}: {e}", path.display()));
                }
            }
        }
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
            folder: "C:/tips".into(),
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

    #[test]
    fn concurrent_writes_never_clobber_each_other() {
        let tmp = tempfile::tempdir().unwrap();
        let st = Storage::new(tmp.path().to_path_buf());
        std::thread::scope(|scope| {
            for t in 0..8 {
                let st = &st;
                scope.spawn(move || {
                    for k in 0..20 {
                        let fired: BTreeSet<String> = [format!("{t}@{k}")].into_iter().collect();
                        st.save_fired(&fired).unwrap();
                    }
                });
            }
        });
        // The survivor is one complete write, and no temp files are left.
        assert_eq!(st.load_fired().len(), 1);
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
