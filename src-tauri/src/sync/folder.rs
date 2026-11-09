//! Tips as Markdown files in one local folder, usually inside OneDrive so the
//! OneDrive client syncs them between machines. The app never talks to a
//! server: it reads and writes files, and OneDrive does the rest.
//!
//! Every file carries a *stamp* (modified time + length). A write names the
//! stamp it was based on; if the file changed on disk since (another PC, an
//! editor), the write is refused as a conflict instead of overwriting it.
//! The check is best effort: another program can still write between the
//! check and the rename, which is the window any file-based sync has.

use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

/// Sub-folder used when the user has not picked one.
pub const DEFAULT_FOLDER_NAME: &str = "Vindictive";
/// Attempts at replacing a file another program (OneDrive, antivirus, an
/// editor) briefly holds open.
const RENAME_ATTEMPTS: u32 = 5;
const RENAME_BACKOFF: Duration = Duration::from_millis(80);
/// Temp files older than this are leftovers of a crash and are removed.
const STALE_TMP: Duration = Duration::from_secs(60);
/// Windows `ERROR_SHARING_VIOLATION`.
const SHARING_VIOLATION: i32 = 32;

/// Serialises check-write-rename and delete within this process, so two
/// quick actions on one tip cannot interleave.
static WRITE_LOCK: Mutex<()> = Mutex::new(());
/// Makes every temp file name unique.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum FolderError {
    #[error("tips folder {0} does not exist or is not a folder")]
    Missing(PathBuf),
    #[error("{0} changed on disk since it was loaded; reloaded, please retry")]
    Conflict(String),
    #[error("a file named {0} already exists in the tips folder")]
    Exists(String),
    #[error("{0} is being written by another program; it is read again on the next scan")]
    Changing(String),
    #[error("{0} is in use by another program (OneDrive or an editor); try again in a moment")]
    InUse(String),
    #[error("refusing path {0:?}: it must stay inside the tips folder")]
    BadPath(String),
    #[error("the folder {0} does not exist (yet); OneDrive may still be syncing it")]
    NoFolder(String),
    #[error("refusing {0}: it is a file, and only folders are opened")]
    NotAFolder(String),
    #[error("{what} {path}: {source}")]
    Io {
        what: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, FolderError>;

/// One tip file: its path relative to the folder and its current stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub stamp: String,
}

impl FileEntry {
    /// True if this file looks like a OneDrive conflict copy of another file
    /// in `listing` (`notes-DESKTOP-AB12CD3.md` next to `notes.md`).
    pub fn is_conflict_copy(&self, listing: &[FileEntry]) -> bool {
        conflict_original(&self.path, listing).is_some()
    }
}

#[derive(Debug, Clone)]
pub struct FolderStore {
    root: PathBuf,
    /// `root` with links resolved, for the containment check in `resolve`.
    canonical_root: PathBuf,
}

impl FolderStore {
    /// A store over an existing folder.
    pub fn open(root: &Path) -> Result<Self> {
        if !root.is_dir() {
            return Err(FolderError::Missing(root.to_path_buf()));
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|_| FolderError::Missing(root.to_path_buf()))?;
        Ok(Self {
            root: root.to_path_buf(),
            canonical_root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every `*.md` file directly in the folder, skipping hidden files and
    /// the temporary files editors and OneDrive leave behind.
    pub fn list(&self) -> Result<Vec<FileEntry>> {
        let entries =
            std::fs::read_dir(&self.root).map_err(|e| io("cannot list", &self.root, e))?;
        let mut files: Vec<FileEntry> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_string();
                if is_tmp_name(&name) {
                    remove_if_stale(&entry.path());
                    return None;
                }
                if !is_tip_name(&name) {
                    return None;
                }
                let meta = entry.metadata().ok().filter(|m| m.is_file())?;
                Some(FileEntry {
                    stamp: stamp_of(&meta),
                    path: name,
                })
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// File text and its stamp. The stamp is taken before and after the
    /// read; if they differ the file was being written and `Changing` is
    /// returned, so stale text is never recorded under a fresh stamp.
    pub fn read_text(&self, rel: &str) -> Result<(String, String)> {
        let path = self.resolve(rel)?;
        let before = self.stamp(&path)?;
        let text = std::fs::read_to_string(&path).map_err(|e| io("cannot read", &path, e))?;
        if self.stamp(&path)? != before {
            return Err(FolderError::Changing(rel.to_string()));
        }
        Ok((text, before))
    }

    pub fn read_bytes(&self, rel: &str) -> Result<Vec<u8>> {
        let path = self.resolve(rel)?;
        std::fs::read(&path).map_err(|e| io("cannot read", &path, e))
    }

    /// Write `text` to `rel` and return the new stamp. `expected` is the stamp
    /// the edit was based on (`None` for a new file); a mismatch is a conflict.
    pub fn write_text(&self, rel: &str, text: &str, expected: Option<&str>) -> Result<String> {
        let path = self.resolve(rel)?;
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        self.check_unchanged(rel, &path, expected)?;
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_file_name(format!(
            ".{}.{}-{n}.tmp",
            file_name(&path),
            std::process::id()
        ));
        let result = write_durably(&tmp, text)
            .map_err(|e| io("cannot write", &tmp, e))
            // A rename keeps the temp file's time and length, so this is the
            // stamp of the new file even if something touches it right after.
            .and_then(|()| self.stamp(&tmp))
            .and_then(|stamp| replace(rel, &tmp, &path).map(|()| stamp));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }

    /// Delete `rel` if it is still the version with stamp `expected`.
    pub fn delete(&self, rel: &str, expected: &str) -> Result<()> {
        let path = self.resolve(rel)?;
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        self.check_unchanged(rel, &path, Some(expected))?;
        retry_in_use(rel, || std::fs::remove_file(&path))
            .map_err(|e| e.unwrap_or_else(|e| io("cannot delete", &path, e)))
    }

    /// Absolute path of `rel`, refusing anything that could leave the folder:
    /// non-plain components, `:` (drive-relative paths and NTFS alternate
    /// data streams), and links or junctions that point outside it.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let bad = || FolderError::BadPath(rel.to_string());
        let rel_path = Path::new(rel);
        let ok = !rel.trim().is_empty()
            && !rel.contains(':')
            && rel_path
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        if !ok {
            return Err(bad());
        }
        let path = self.root.join(rel_path);
        // The deepest part that exists must resolve to inside the folder; a
        // file still to be created is checked through its parent folder.
        let mut probe = path.as_path();
        loop {
            match probe.canonicalize() {
                Ok(real) if real.starts_with(&self.canonical_root) => return Ok(path),
                Ok(_) => return Err(bad()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    if probe == self.root {
                        return Err(FolderError::Missing(self.root.clone()));
                    }
                    probe = probe.parent().ok_or_else(bad)?;
                }
                Err(e) => return Err(io("cannot inspect", probe, e)),
            }
        }
    }

    /// Canonical path of the existing *folder* `rel`, checked to be inside
    /// the canonical tips folder once links and junctions are followed.
    /// Files are refused: the result is handed to the shell opener, which
    /// would run an `.exe`, `.lnk` or `.bat`. Used for agent results only.
    pub fn resolve_existing_dir(&self, rel: &str) -> Result<PathBuf> {
        let path = self.resolve(rel)?;
        let real = std::fs::canonicalize(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FolderError::NoFolder(rel.to_string()),
            _ => io("cannot inspect", &path, e),
        })?;
        if !real.starts_with(&self.canonical_root) {
            return Err(FolderError::BadPath(rel.to_string()));
        }
        if !real.is_dir() {
            return Err(FolderError::NotAFolder(rel.to_string()));
        }
        Ok(plain_path(&real))
    }

    fn check_unchanged(&self, rel: &str, path: &Path, expected: Option<&str>) -> Result<()> {
        let current = match std::fs::metadata(path) {
            Ok(meta) => Some(stamp_of(&meta)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io("cannot inspect", path, e)),
        };
        match (current.as_deref(), expected) {
            (current, expected) if current == expected => Ok(()),
            (Some(_), None) => Err(FolderError::Exists(rel.to_string())),
            _ => Err(FolderError::Conflict(rel.to_string())),
        }
    }

    fn stamp(&self, path: &Path) -> Result<String> {
        std::fs::metadata(path)
            .map(|m| stamp_of(&m))
            .map_err(|e| io("cannot inspect", path, e))
    }
}

/// The folder to use when the user has not picked one: `Vindictive` inside
/// OneDrive when OneDrive is set up, otherwise inside `fallback` (Documents).
pub fn default_folder(onedrive: Option<&str>, fallback: &Path) -> PathBuf {
    onedrive
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback.to_path_buf())
        .join(DEFAULT_FOLDER_NAME)
}

/// The OneDrive root from the environment, personal account first.
pub fn onedrive_root() -> Option<String> {
    ["OneDriveConsumer", "OneDrive", "OneDriveCommercial"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .find(|p| !p.trim().is_empty())
}

/// A canonical path without the Windows verbatim prefix (`\\?\C:\...` to
/// `C:\...`, `\\?\UNC\srv\share` to `\\srv\share`), which Explorer does not
/// accept. Anything else is returned as it is.
fn plain_path(path: &Path) -> PathBuf {
    match path.to_str() {
        Some(s) if s.starts_with(r"\\?\UNC\") => PathBuf::from(format!(r"\\{}", &s[8..])),
        Some(s) if s.starts_with(r"\\?\") && s.as_bytes().get(5) == Some(&b':') => {
            PathBuf::from(&s[4..])
        }
        _ => path.to_path_buf(),
    }
}

/// Move `tmp` over `path`, retrying while another program holds a handle.
fn replace(rel: &str, tmp: &Path, path: &Path) -> Result<()> {
    retry_in_use(rel, || std::fs::rename(tmp, path))
        .map_err(|e| e.unwrap_or_else(|e| io("cannot replace", path, e)))
}

/// Run `op`, retrying a few times on "in use" errors. The error is
/// `Err(Ok(InUse))` when it never got free, `Err(Err(io))` otherwise.
fn retry_in_use(
    rel: &str,
    mut op: impl FnMut() -> std::io::Result<()>,
) -> std::result::Result<(), std::result::Result<FolderError, std::io::Error>> {
    for attempt in 1..=RENAME_ATTEMPTS {
        match op() {
            Ok(()) => return Ok(()),
            Err(e) if is_in_use(&e) && attempt < RENAME_ATTEMPTS => {
                std::thread::sleep(RENAME_BACKOFF * attempt);
            }
            Err(e) if is_in_use(&e) => return Err(Ok(FolderError::InUse(rel.to_string()))),
            Err(e) => return Err(Err(e)),
        }
    }
    Err(Ok(FolderError::InUse(rel.to_string())))
}

fn is_in_use(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(SHARING_VIOLATION)
}

/// `.name.md.<pid>-<n>.tmp`, written by `write_text`.
fn is_tmp_name(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(".tmp")
}

fn remove_if_stale(path: &Path) {
    let old = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age > STALE_TMP);
    if old {
        let _ = std::fs::remove_file(path);
    }
}

fn is_tip_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".md") && !name.starts_with('.') && !name.starts_with('~')
}

/// The file in `listing` that `name` is a OneDrive conflict copy of, if any.
///
/// OneDrive keeps both sides of a conflict by renaming one to
/// `<stem>-<ComputerName>[-<n>].<ext>`; Explorer-style copies are
/// `<stem> (<n>).<ext>`. A name only counts when the original is also in
/// the listing, so an ordinary file like `plan-B.md` is not flagged unless
/// `plan.md` exists too.
pub fn conflict_original<'a>(name: &str, listing: &'a [FileEntry]) -> Option<&'a str> {
    let (stem, ext) = name.rsplit_once('.')?;
    conflict_stems(stem).into_iter().find_map(|base| {
        let original = format!("{base}.{ext}");
        listing
            .iter()
            .find(|f| f.path.eq_ignore_ascii_case(&original))
            .map(|f| f.path.as_str())
    })
}

/// Every conflict copy in `listing`, paired with its original.
pub fn conflict_copies(listing: &[FileEntry]) -> Vec<(&str, &str)> {
    listing
        .iter()
        .filter_map(|f| Some((f.path.as_str(), conflict_original(&f.path, listing)?)))
        .collect()
}

/// One line for the sync summary, `None` when there are no conflict copies.
pub fn conflict_summary(listing: &[FileEntry]) -> Option<String> {
    let copies = conflict_copies(listing);
    let (first, original) = copies.first()?;
    let more = match copies.len() {
        1 => String::new(),
        n => format!(" and {} more", n - 1),
    };
    Some(format!(
        "OneDrive kept a conflict copy: {first} (of {original}){more}; merge and delete the copy"
    ))
}

/// Stems `stem` could be a conflict copy of, most specific first.
fn conflict_stems(stem: &str) -> Vec<&str> {
    let mut out = Vec::new();
    // `notes (2)`
    if let Some(base) = stem.strip_suffix(')').and_then(|s| {
        let (base, n) = s.rsplit_once(" (")?;
        (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then_some(base)
    }) {
        out.push(base);
    }
    // `notes-DESKTOP-AB12CD3`, `notes-WORKPC-2`: try every `-` as the split.
    for (i, _) in stem.match_indices('-') {
        let (base, suffix) = (&stem[..i], &stem[i + 1..]);
        if !base.is_empty() && is_computer_name_suffix(suffix) {
            out.push(base);
        }
    }
    out
}

/// A Windows computer name as OneDrive writes it, upper case, optionally
/// followed by `-<n>`: at most 15 characters of `A-Z`, `0-9` and `-`,
/// with at least one letter.
fn is_computer_name_suffix(suffix: &str) -> bool {
    let host = match suffix.rsplit_once('-') {
        Some((host, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => suffix,
    };
    let shaped = |s: &str| {
        (1..=15).contains(&s.len())
            && !s.starts_with('-')
            && !s.ends_with('-')
            && s.bytes().any(|b| b.is_ascii_uppercase())
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
    };
    shaped(host) || shaped(suffix)
}

/// Write `text` to a new file and flush it to disk before it is renamed
/// over the original, so a crash cannot leave an empty tip behind.
fn write_durably(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// Modification time (full precision, nanoseconds since the epoch) and
/// length. The filesystem's own clock is coarser than that (NTFS updates
/// in 100 ns units but can lag, FAT in 2 s), so two quick writes of the
/// same length can share a stamp; callers that must notice every save, like
/// the broken-file retry, also retry on a timer.
fn stamp_of(meta: &std::fs::Metadata) -> String {
    let nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos}-{}", meta.len())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn io(what: &'static str, path: &Path, source: std::io::Error) -> FolderError {
    FolderError::Io {
        what,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (FolderStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (FolderStore::open(dir.path()).unwrap(), dir)
    }

    #[test]
    fn missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            FolderStore::open(&dir.path().join("nope")),
            Err(FolderError::Missing(_))
        ));
    }

    #[test]
    fn lists_only_visible_markdown_files() {
        let (s, dir) = store();
        for name in [
            "b.md",
            "a.MD",
            ".a.md.tmp",
            ".hidden.md",
            "~$lock.md",
            "x.txt",
        ] {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        std::fs::create_dir(dir.path().join("figures.md")).unwrap();
        let names: Vec<String> = s.list().unwrap().into_iter().map(|f| f.path).collect();
        assert_eq!(names, vec!["a.MD", "b.md"]);
    }

    #[test]
    fn write_then_read_roundtrips_with_matching_stamps() {
        let (s, _dir) = store();
        let stamp = s.write_text("a.md", "hello", None).unwrap();
        let (text, read_stamp) = s.read_text("a.md").unwrap();
        assert_eq!(text, "hello");
        assert_eq!(read_stamp, stamp);
        assert_eq!(s.list().unwrap()[0].stamp, stamp);
        let next = s.write_text("a.md", "hello again", Some(&stamp)).unwrap();
        assert_ne!(next, stamp);
        assert_eq!(s.read_text("a.md").unwrap().0, "hello again");
    }

    #[test]
    fn stale_or_missing_base_is_a_conflict() {
        let (s, dir) = store();
        let stamp = s.write_text("a.md", "one", None).unwrap();
        // Another machine (via OneDrive) rewrites the file.
        std::fs::write(dir.path().join("a.md"), "changed elsewhere!").unwrap();
        assert!(matches!(
            s.write_text("a.md", "mine", Some(&stamp)),
            Err(FolderError::Conflict(_))
        ));
        assert!(matches!(
            s.write_text("a.md", "new", None),
            Err(FolderError::Exists(_))
        ));
        assert_eq!(s.read_text("a.md").unwrap().0, "changed elsewhere!");
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn stale_temp_files_are_swept_fresh_ones_kept() {
        let (s, dir) = store();
        let fresh = dir.path().join(".a.md.1-1.tmp");
        std::fs::write(&fresh, "x").unwrap();
        let stale = dir.path().join(".b.md.1-2.tmp");
        let file = std::fs::File::create(&stale).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
        drop(file);
        assert!(s.list().unwrap().is_empty());
        assert!(fresh.exists());
        assert!(!stale.exists());
    }

    #[test]
    fn concurrent_writes_never_collide_on_temp_files() {
        let (s, _dir) = store();
        let first = s.write_text("a.md", "0", None).unwrap();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let s = s.clone();
                let base = first.clone();
                std::thread::spawn(move || s.write_text("a.md", &"x".repeat(i + 2), Some(&base)))
            })
            .collect();
        let ok = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(Result::is_ok)
            .count();
        assert_eq!(ok, 1, "exactly one write wins; the rest see a conflict");
    }

    #[test]
    fn delete_checks_the_stamp() {
        let (s, dir) = store();
        let stamp = s.write_text("a.md", "one", None).unwrap();
        assert!(s.delete("a.md", "0-0").is_err());
        s.delete("a.md", &stamp).unwrap();
        assert!(!dir.path().join("a.md").exists());
    }

    #[test]
    fn paths_cannot_escape() {
        let (s, _dir) = store();
        for bad in ["", "../x.md", "/etc/x", "a/../../x", "./a.md", "C:\\x.md"] {
            assert!(s.resolve(bad).is_err(), "{bad}");
        }
        assert!(s.resolve("figures/plot.png").is_ok());
    }

    #[test]
    fn existing_dirs_must_be_folders_inside_the_tips_folder() {
        let (s, dir) = store();
        let survey = dir.path().join("agent").join("survey");
        std::fs::create_dir_all(&survey).unwrap();
        std::fs::write(survey.join("result.md"), "x").unwrap();
        std::fs::write(survey.join("setup.exe"), "MZ").unwrap();
        // The canonical path, in a form Explorer accepts.
        let got = s.resolve_existing_dir("agent/survey").unwrap();
        assert_eq!(got, plain_path(&std::fs::canonicalize(&survey).unwrap()));
        assert!(got.is_absolute() && !got.to_string_lossy().starts_with(r"\\?\"));
        // Files are never handed to the opener.
        for file in ["agent/survey/result.md", "agent/survey/setup.exe"] {
            assert!(
                matches!(
                    s.resolve_existing_dir(file),
                    Err(FolderError::NotAFolder(_))
                ),
                "{file}"
            );
        }
        let missing = s.resolve_existing_dir("agent/missing").unwrap_err();
        assert!(matches!(missing, FolderError::NoFolder(_)));
        assert!(missing.to_string().contains("does not exist"), "{missing}");
        for bad in ["../x", "/etc", "agent/../../x", r"C:\Windows"] {
            assert!(
                matches!(s.resolve_existing_dir(bad), Err(FolderError::BadPath(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn plain_paths_drop_the_verbatim_prefix() {
        assert_eq!(
            plain_path(Path::new(r"\\?\C:\Users\me\Tips\agent\x")),
            PathBuf::from(r"C:\Users\me\Tips\agent\x")
        );
        assert_eq!(
            plain_path(Path::new(r"\\?\UNC\nas\share\agent")),
            PathBuf::from(r"\\nas\share\agent")
        );
        assert_eq!(
            plain_path(Path::new(r"\\?\Volume{abc}\x")),
            PathBuf::from(r"\\?\Volume{abc}\x")
        );
        assert_eq!(
            plain_path(Path::new("/tmp/tips")),
            PathBuf::from("/tmp/tips")
        );
    }

    #[test]
    fn colons_and_streams_are_refused() {
        let (s, _dir) = store();
        for bad in ["a.md:evil", "a.md::$DATA", "C:x.md", "sub/b:c.md"] {
            assert!(
                matches!(s.resolve(bad), Err(FolderError::BadPath(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn existing_and_new_files_resolve_inside_the_folder() {
        let (s, dir) = store();
        std::fs::write(dir.path().join("a.md"), "x").unwrap();
        assert_eq!(s.resolve("a.md").unwrap(), dir.path().join("a.md"));
        assert_eq!(s.resolve("new.md").unwrap(), dir.path().join("new.md"));
        assert_eq!(
            s.resolve("no/such/dir/x.md").unwrap(),
            dir.path().join("no/such/dir/x.md")
        );
    }

    #[cfg(windows)]
    #[test]
    fn links_out_of_the_folder_are_refused() {
        let (s, dir) = store();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.md"), "x").unwrap();
        // Creating symlinks needs Developer Mode or admin; skip without it.
        if std::os::windows::fs::symlink_dir(outside.path(), dir.path().join("out")).is_err() {
            return;
        }
        assert!(matches!(
            s.resolve("out/secret.md"),
            Err(FolderError::BadPath(_))
        ));
        assert!(matches!(
            s.resolve("out/new.md"),
            Err(FolderError::BadPath(_))
        ));
    }

    fn entries(names: &[&str]) -> Vec<FileEntry> {
        names
            .iter()
            .map(|n| FileEntry {
                path: n.to_string(),
                stamp: "1-1".into(),
            })
            .collect()
    }

    #[test]
    fn detects_onedrive_conflict_copies() {
        let listing = entries(&[
            "notes.md",
            "notes-DESKTOP-AB12CD3.md",
            "plan.md",
            "plan-LAPTOP-9X-2.md",
            "todo (1).md",
            "todo.md",
            "2026-10-01-coil.md",
            "2026-10-01-coil-check.md",
            "orphan-DESKTOP-ZZ.md",
            "report-B.md",
        ]);
        let copies = conflict_copies(&listing);
        assert_eq!(
            copies,
            vec![
                ("notes-DESKTOP-AB12CD3.md", "notes.md"),
                ("plan-LAPTOP-9X-2.md", "plan.md"),
                ("todo (1).md", "todo.md"),
            ]
        );
        assert!(listing[1].is_conflict_copy(&listing));
        assert!(!listing[0].is_conflict_copy(&listing));
        assert!(
            !listing[7].is_conflict_copy(&listing),
            "lower-case slug is not a computer name"
        );
        assert!(!listing[8].is_conflict_copy(&listing), "no original");
        let summary = conflict_summary(&listing).unwrap();
        assert!(summary.contains("notes-DESKTOP-AB12CD3.md (of notes.md) and 2 more"));
        assert_eq!(conflict_summary(&entries(&["a.md"])), None);
    }

    #[test]
    fn computer_name_shapes() {
        for ok in ["DESKTOP-AB12CD3", "WORKPC", "PC-2", "LAPTOP-9X-12"] {
            assert!(is_computer_name_suffix(ok), "{ok}");
        }
        for no in ["check", "Desktop", "123", "-X", "X-", "ABCDEFGHIJKLMNOPQ"] {
            assert!(!is_computer_name_suffix(no), "{no}");
        }
    }

    #[test]
    fn writes_leave_no_temp_files_behind() {
        let (s, dir) = store();
        s.write_text("a.md", "durable", None).unwrap();
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.md"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.md")).unwrap(),
            "durable"
        );
    }

    #[test]
    fn default_folder_prefers_onedrive() {
        let docs = Path::new("C:/Users/me/Documents");
        assert_eq!(
            default_folder(Some("C:/Users/me/OneDrive"), docs),
            Path::new("C:/Users/me/OneDrive").join(DEFAULT_FOLDER_NAME)
        );
        assert_eq!(
            default_folder(Some("  "), docs),
            docs.join(DEFAULT_FOLDER_NAME)
        );
        assert_eq!(default_folder(None, docs), docs.join(DEFAULT_FOLDER_NAME));
    }
}
