//! The `Tip` domain object and its Markdown + YAML frontmatter representation.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::frontmatter::{join, split, FrontmatterError};
use super::timeparse::{format_dt, is_offset, parse_due, parse_remind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Task,
    Deadline,
    Note,
    Reading,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Open,
    Done,
}

// The enums are written lowercase but read case-insensitively (`kind: Task`),
// and an empty value (`kind:`) means the default.
lenient::enum_deserialize!(Kind {
    "task" => Task,
    "deadline" => Deadline,
    "note" => Note,
    "reading" => Reading,
    "event" => Event,
});
lenient::enum_deserialize!(Priority {
    "low" => Low,
    "normal" => Normal,
    "high" => High,
});
lenient::enum_deserialize!(Status {
    "open" => Open,
    "done" => Done,
});

/// Forgiving readers for hand-written YAML: a number or boolean where text is
/// expected is taken as its text (`title: 2024`), and a single value where a
/// list is expected is a one-item list (`remind: 2026-10-14`). Serialisation
/// is untouched, so files are still written in the canonical form.
mod lenient {
    use serde::de::{Deserialize, Deserializer, Error};
    use serde_yaml::Value;

    /// Text of a scalar; `None` for null; an error for a list or mapping.
    pub fn scalar_text<E: Error>(value: Value) -> Result<Option<String>, E> {
        match value {
            Value::Null => Ok(None),
            Value::Bool(b) => Ok(Some(b.to_string())),
            Value::Number(n) => Ok(Some(n.to_string())),
            Value::String(s) => Ok(Some(s)),
            Value::Tagged(t) => scalar_text(t.value),
            Value::Sequence(_) => Err(E::custom("expected a single value, found a list")),
            Value::Mapping(_) => Err(E::custom("expected a single value, found a mapping")),
        }
    }

    pub fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        Ok(scalar_text(Value::deserialize(d)?)?.unwrap_or_default())
    }

    pub fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        scalar_text(Value::deserialize(d)?)
    }

    /// `arxiv: 2401.10000` unquoted is a YAML float, and a float has lost the
    /// trailing zeros. Rebuild the id from the number; text is kept as is.
    pub fn arxiv<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        match Value::deserialize(d)? {
            Value::Number(n) => Ok(Some(arxiv_from_number(&n.to_string()))),
            other => scalar_text(other),
        }
    }

    /// `2401.1` → `2401.10000`, `704.0001` → `0704.0001`: the YYMM part has
    /// four digits, the sequence four before 2015-01 and five from then on.
    /// Anything that does not fit that shape is returned unchanged.
    pub fn arxiv_from_number(text: &str) -> String {
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        let Some((yymm, seq)) = text.split_once('.') else {
            return text.to_string();
        };
        if !digits(yymm) || !digits(seq) || yymm.len() > 4 {
            return text.to_string();
        }
        let yymm = format!("{yymm:0>4}");
        let width = if yymm.as_str() < "1501" { 4 } else { 5 };
        if seq.len() > width {
            return text.to_string();
        }
        format!("{yymm}.{seq:0<width$}")
    }

    pub fn string_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
        list_text(Value::deserialize(d)?)
    }

    fn list_text<E: Error>(value: Value) -> Result<Vec<String>, E> {
        match value {
            Value::Sequence(items) => Ok(items
                .into_iter()
                .map(scalar_text)
                .collect::<Result<Vec<_>, E>>()?
                .into_iter()
                .flatten()
                .collect()),
            Value::Tagged(t) => list_text(t.value),
            other => Ok(scalar_text(other)?.into_iter().collect()),
        }
    }

    macro_rules! enum_deserialize {
        ($ty:ident { $($name:literal => $variant:ident),+ $(,)? }) => {
            impl<'de> Deserialize<'de> for $ty {
                fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    use serde::de::Error;
                    let value = serde_yaml::Value::deserialize(d)?;
                    let Some(text) = lenient::scalar_text::<D::Error>(value)? else {
                        return Ok($ty::default());
                    };
                    match text.trim().to_lowercase().as_str() {
                        "" => Ok($ty::default()),
                        $($name => Ok($ty::$variant),)+
                        _ => Err(D::Error::unknown_variant(&text, &[$($name),+])),
                    }
                }
            }
        };
    }
    pub(super) use enum_deserialize;
}

/// Bibliographic metadata fetched from arXiv or Crossref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Paper {
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Raw YAML frontmatter exactly as written in the file. Dates stay strings
/// here so that a hand-written `2026-10-15` survives a round trip untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FrontMatter {
    #[serde(default, deserialize_with = "lenient::string")]
    pub title: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub status: Status,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub due: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient::string_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub remind: Vec<String>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub location: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient::string_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub links: Vec<String>,
    #[serde(
        default,
        deserialize_with = "lenient::string_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub images: Vec<String>,
    #[serde(
        default,
        deserialize_with = "lenient::string_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub tags: Vec<String>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub repeat: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub color: Option<String>,
    /// Tile size override: `sm`, `md` or `wide`. Absent means automatic.
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub size: Option<String>,
    /// Manual board position (set by drag and drop); absent means by score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
    #[serde(
        default,
        deserialize_with = "lenient::arxiv",
        skip_serializing_if = "Option::is_none"
    )]
    pub arxiv: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<Paper>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub snoozed_until: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub done_at: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient::opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub created: Option<String>,
    /// Words or phrases to highlight on the tile and in the body
    /// (case-insensitive), on top of the global list in Settings.
    #[serde(
        default,
        deserialize_with = "lenient::string_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub highlight: Vec<String>,
    /// Any keys we do not understand are preserved verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

/// A tip as the UI sees it: frontmatter plus resolved timestamps plus where it
/// lives in the tips folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "TipJson", from = "TipJson")]
pub struct Tip {
    /// Stable identifier: the file name without `.md`.
    pub id: String,
    /// Path relative to the tips folder, e.g. `2026-10-01-camera-ready.md`.
    pub path: String,
    /// Change stamp of the file as last read (see `sync::folder`). A write
    /// passes it back so that a file changed on disk is a conflict, not lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(flatten)]
    pub front: FrontMatter,
    pub body: String,
    /// Parsed `due`, local naive time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<NaiveDateTime>,
    /// Parsed `remind` entries, local naive time, sorted ascending.
    #[serde(default)]
    pub remind_at: Vec<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until_at: Option<NaiveDateTime>,
    /// Problems found while resolving the frontmatter, e.g. a `due` that does
    /// not parse. Derived, never written to the file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Top-level JSON keys of a `Tip` besides the frontmatter ones.
const TIP_KEYS: [&str; 9] = [
    "id",
    "path",
    "sha",
    "body",
    "due_at",
    "remind_at",
    "snoozed_until_at",
    "warnings",
    "shadowed",
];

/// JSON shape of a `Tip` (IPC and the offline cache). The frontmatter is
/// flattened into it, so an unknown frontmatter key named like a tip field
/// (`id: 20240101` from Obsidian, say) would emit that key twice. Such keys
/// travel in `shadowed` instead and are put back on the way in.
#[derive(Serialize, Deserialize)]
struct TipJson {
    id: String,
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha: Option<String>,
    #[serde(flatten)]
    front: FrontMatter,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    shadowed: BTreeMap<String, serde_yaml::Value>,
    body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due_at: Option<NaiveDateTime>,
    #[serde(default)]
    remind_at: Vec<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    snoozed_until_at: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

impl From<Tip> for TipJson {
    fn from(tip: Tip) -> Self {
        let mut front = tip.front;
        let (shadowed, extra) = std::mem::take(&mut front.extra)
            .into_iter()
            .partition(|(k, _)| TIP_KEYS.contains(&k.as_str()));
        front.extra = extra;
        TipJson {
            id: tip.id,
            path: tip.path,
            sha: tip.sha,
            front,
            shadowed,
            body: tip.body,
            due_at: tip.due_at,
            remind_at: tip.remind_at,
            snoozed_until_at: tip.snoozed_until_at,
            warnings: tip.warnings,
        }
    }
}

impl From<TipJson> for Tip {
    fn from(json: TipJson) -> Self {
        let mut front = json.front;
        front.extra.extend(json.shadowed);
        Tip {
            id: json.id,
            path: json.path,
            sha: json.sha,
            front,
            body: json.body,
            due_at: json.due_at,
            remind_at: json.remind_at,
            snoozed_until_at: json.snoozed_until_at,
            warnings: json.warnings,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TipError {
    #[error("{0}")]
    Frontmatter(#[from] FrontmatterError),
    #[error("invalid YAML frontmatter: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("tip has no title")]
    MissingTitle,
}

impl Tip {
    /// Parse a Markdown file with YAML frontmatter.
    pub fn parse(path: &str, sha: Option<String>, text: &str) -> Result<Tip, TipError> {
        let (yaml, body) = split(text)?;
        let front: FrontMatter = serde_yaml::from_str(yaml)?;
        if front.title.trim().is_empty() {
            return Err(TipError::MissingTitle);
        }
        // Only the source text shows whether `arxiv` was written unquoted.
        let arxiv_is_number = front.arxiv.is_some()
            && serde_yaml::from_str::<serde_yaml::Value>(yaml)
                .is_ok_and(|v| v.get("arxiv").is_some_and(serde_yaml::Value::is_number));
        let mut tip = Tip::from_parts(path, sha, front, body.to_string());
        if arxiv_is_number {
            tip.warnings
                .push("arxiv: unquoted id read as a number; quote it in the file".into());
        }
        Ok(tip)
    }

    pub fn from_parts(path: &str, sha: Option<String>, front: FrontMatter, body: String) -> Tip {
        let id = id_from_path(path);
        let mut warnings = Vec::new();
        let mut resolve = |key: &str, raw: Option<&str>| {
            let parsed = raw.and_then(parse_due);
            if let (Some(raw), None) = (raw, parsed) {
                warnings.push(format!("{key}: cannot parse {raw:?}"));
            }
            parsed
        };
        let due_at = resolve("due", front.due.as_deref());
        let snoozed_until_at = resolve("snoozed_until", front.snoozed_until.as_deref());
        let mut remind_at = Vec::new();
        for raw in &front.remind {
            match parse_remind(raw, due_at) {
                Some(at) => remind_at.push(at),
                None if is_offset(raw) && due_at.is_none() => {
                    warnings.push(format!("remind: {raw:?} needs a due date"))
                }
                None if is_offset(raw) => warnings.push(format!("remind: {raw:?} is out of range")),
                None => warnings.push(format!("remind: cannot parse {raw:?}")),
            }
        }
        remind_at.sort();
        remind_at.dedup();
        if let Some(rule) = front.repeat.as_deref() {
            if super::recur::parse(rule).is_none() {
                warnings.push(format!("repeat: cannot parse {rule:?}"));
            }
        }
        Tip {
            id,
            path: path.to_string(),
            sha,
            front,
            body,
            due_at,
            remind_at,
            snoozed_until_at,
            warnings,
        }
    }

    /// Serialize back to Markdown with YAML frontmatter.
    pub fn to_markdown(&self) -> Result<String, TipError> {
        let yaml = serde_yaml::to_string(&self.front)?;
        Ok(join(&yaml, &self.body))
    }

    /// Return a copy with a new frontmatter. Derived timestamps are recomputed.
    pub fn with_front(&self, front: FrontMatter) -> Tip {
        Tip::from_parts(&self.path, self.sha.clone(), front, self.body.clone())
    }

    pub fn is_open(&self) -> bool {
        self.front.status == Status::Open
    }

    /// True if the tip is snoozed at `now`.
    pub fn is_snoozed(&self, now: NaiveDateTime) -> bool {
        matches!(self.snoozed_until_at, Some(t) if t > now)
    }

    /// Title to show on the tile: prefers a fetched paper title for reading tips.
    pub fn display_title(&self) -> &str {
        match (&self.front.kind, &self.front.paper) {
            (Kind::Reading, Some(p)) if !p.title.is_empty() => &p.title,
            _ => &self.front.title,
        }
    }

    /// Mark done at `now`. Recurring tips roll forward instead of closing:
    /// past the current due date, so finishing early still advances it.
    pub fn complete(&self, now: NaiveDateTime) -> Tip {
        let mut front = self.front.clone();
        front.snoozed_until = None;
        front.done_at = Some(format_dt(now));
        let after = self.due_at.map_or(now, |due| due.max(now));
        let rolled = front
            .repeat
            .as_deref()
            .and_then(|rule| super::recur::next_occurrence(rule, after));
        match rolled {
            Some(next_due) => {
                let shift = self.due_at.map(|d| next_due - d);
                front.due = Some(format_dt(next_due));
                front.remind = self
                    .remind_at
                    .iter()
                    .filter_map(|r| r.checked_add_signed(shift?))
                    .map(format_dt)
                    .collect();
            }
            None => front.status = Status::Done,
        }
        self.with_front(front)
    }

    pub fn reopen(&self) -> Tip {
        let mut front = self.front.clone();
        front.status = Status::Open;
        front.done_at = None;
        self.with_front(front)
    }

    pub fn snooze_until(&self, until: NaiveDateTime) -> Tip {
        let mut front = self.front.clone();
        front.snoozed_until = Some(format_dt(until));
        self.with_front(front)
    }

    pub fn with_paper(&self, paper: Paper) -> Tip {
        let mut front = self.front.clone();
        front.paper = Some(paper);
        self.with_front(front)
    }
}

pub fn id_from_path(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name).to_string()
}

const MAX_SLUG_BYTES: usize = 48;

/// Turn a title into a safe, short file stem.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in title.chars().flat_map(|c| c.to_lowercase()) {
        // Letters and digits in any script, so `读论文` gives a real slug.
        let next = if c.is_alphanumeric() {
            c
        } else if !last_dash {
            '-'
        } else {
            continue;
        };
        if out.len() + next.len_utf8() > MAX_SLUG_BYTES {
            break;
        }
        out.push(next);
        last_dash = next == '-';
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "tip".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    const SAMPLE: &str = "---\ntitle: Submit camera-ready\nkind: deadline\npriority: high\ndue: 2026-10-15\nremind:\n  - 2026-10-14 09:00\n  - -1d\nlocation: Lab 302\nlinks:\n  - https://example.org\ntags: [ecce, paper]\ncustom_key: keep me\n---\n\nBody **text**.\n";

    #[test]
    fn parses_frontmatter_and_body() {
        let tip = Tip::parse("tips/2026-camera.md", Some("abc".into()), SAMPLE).unwrap();
        assert_eq!(tip.id, "2026-camera");
        assert_eq!(tip.front.title, "Submit camera-ready");
        assert_eq!(tip.front.kind, Kind::Deadline);
        assert_eq!(tip.front.priority, Priority::High);
        assert_eq!(tip.due_at, Some(dt("2026-10-15 23:59")));
        assert_eq!(
            tip.remind_at,
            vec![dt("2026-10-14 09:00"), dt("2026-10-14 23:59")]
        );
        assert_eq!(tip.body.trim(), "Body **text**.");
        assert_eq!(
            tip.front.extra["custom_key"],
            serde_yaml::Value::String("keep me".into())
        );
    }

    #[test]
    fn roundtrip_preserves_fields() {
        let tip = Tip::parse("tips/a.md", None, SAMPLE).unwrap();
        let text = tip.to_markdown().unwrap();
        let again = Tip::parse("tips/a.md", None, &text).unwrap();
        assert_eq!(tip.front, again.front);
        assert_eq!(tip.body, again.body);
    }

    #[test]
    fn missing_title_is_error() {
        let err = Tip::parse("x.md", None, "---\nkind: task\n---\nhi").unwrap_err();
        assert!(matches!(err, TipError::MissingTitle));
    }

    #[test]
    fn complete_closes_plain_tip() {
        let tip = Tip::parse("x.md", None, SAMPLE).unwrap();
        let done = tip.complete(dt("2026-10-01 10:00"));
        assert_eq!(done.front.status, Status::Done);
        assert_eq!(done.front.done_at.as_deref(), Some("2026-10-01 10:00"));
        assert_eq!(
            tip.front.status,
            Status::Open,
            "original must not be mutated"
        );
    }

    #[test]
    fn frontmatter_keys_named_like_tip_fields_survive_json() {
        let text = "---
title: Zettel
id: 20240101
path: notes/z.md
---
body
";
        let tip = Tip::parse("tips/zettel.md", Some("abc".into()), text).unwrap();
        let json = serde_json::to_string(&tip).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["id"], "zettel", "the file id wins in JSON");
        assert_eq!(value["path"], "tips/zettel.md");
        let back: Tip = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tip);
        let md = back.to_markdown().unwrap();
        assert!(md.contains("id: 20240101"), "{md}");
        assert!(md.contains("path: notes/z.md"), "{md}");
    }

    #[test]
    fn complete_rolls_recurring_tip_forward() {
        let text = "---\ntitle: Group meeting\nkind: event\ndue: 2026-10-05 10:00\nremind: [2026-10-05 09:30]\nrepeat: weekly on mon at 10:00\n---\n";
        let tip = Tip::parse("gm.md", None, text).unwrap();
        let done = tip.complete(dt("2026-10-05 11:00"));
        assert_eq!(done.front.status, Status::Open);
        assert_eq!(done.due_at, Some(dt("2026-10-12 10:00")));
        assert_eq!(done.remind_at, vec![dt("2026-10-12 09:30")]);
        // Done before the meeting (Sunday evening) still moves to next week.
        let early = tip.complete(dt("2026-10-04 20:00"));
        assert_eq!(early.due_at, Some(dt("2026-10-12 10:00")));
    }

    #[test]
    fn snooze_and_reopen() {
        let tip = Tip::parse("x.md", None, SAMPLE).unwrap();
        let s = tip.snooze_until(dt("2026-10-01 12:00"));
        assert!(s.is_snoozed(dt("2026-10-01 11:00")));
        assert!(!s.is_snoozed(dt("2026-10-01 12:00")));
        let r = tip.complete(dt("2026-10-01 10:00")).reopen();
        assert_eq!(r.front.status, Status::Open);
        assert!(r.front.done_at.is_none());
    }

    #[test]
    fn unquoted_arxiv_ids_are_rebuilt_and_flagged() {
        let parse = |v: &str| {
            Tip::parse("p.md", None, &format!("---\ntitle: t\narxiv: {v}\n---\n")).unwrap()
        };
        let warning = "arxiv: unquoted id read as a number; quote it in the file";
        let tip = parse("2401.10000");
        assert_eq!(
            tip.front.arxiv.as_deref(),
            Some("2401.10000"),
            "5-digit era"
        );
        assert_eq!(tip.warnings, vec![warning]);
        let tip = parse("0704.0001");
        assert_eq!(tip.front.arxiv.as_deref(), Some("0704.0001"), "4-digit era");
        assert_eq!(tip.warnings, vec![warning]);
        assert_eq!(parse("1412.1230").front.arxiv.as_deref(), Some("1412.1230"));
        assert_eq!(
            parse("1501.12300").front.arxiv.as_deref(),
            Some("1501.12300")
        );
        // Quoted: kept exactly, no warning.
        let tip = parse("'2401.10000'");
        assert_eq!(tip.front.arxiv.as_deref(), Some("2401.10000"));
        assert!(tip.warnings.is_empty());
        let tip = parse("arXiv:2401.10000v2");
        assert_eq!(tip.front.arxiv.as_deref(), Some("arXiv:2401.10000v2"));
        assert!(tip.warnings.is_empty());
        // Numbers that are not an id shape come back as plain text.
        assert_eq!(lenient::arxiv_from_number("2401.1"), "2401.10000");
        assert_eq!(lenient::arxiv_from_number("704.0001"), "0704.0001");
        assert_eq!(lenient::arxiv_from_number("-2401.1"), "-2401.1");
        assert_eq!(lenient::arxiv_from_number("2401.123456"), "2401.123456");
        assert_eq!(lenient::arxiv_from_number("1412.12345"), "1412.12345");
        assert_eq!(lenient::arxiv_from_number("12345.1"), "12345.1");
        assert_eq!(lenient::arxiv_from_number("2401"), "2401");
        assert_eq!(lenient::arxiv_from_number("1e20"), "1e20");
        // Once the app writes the file back, the id is quoted and stays put.
        let md = parse("2401.10000").to_markdown().unwrap();
        let again = Tip::parse("p.md", None, &md).unwrap();
        assert_eq!(again.front.arxiv.as_deref(), Some("2401.10000"));
        assert!(again.warnings.is_empty(), "{md}");
    }

    #[test]
    fn bad_repeat_is_a_warning() {
        let text = "---\ntitle: t\ndue: 2026-10-05 10:00\nrepeat: fortnightly\n---\n";
        let tip = Tip::parse("r.md", None, text).unwrap();
        assert_eq!(tip.warnings, vec![r#"repeat: cannot parse "fortnightly""#]);
        let ok = "---\ntitle: t\nrepeat: weekly on mon\n---\n";
        assert!(Tip::parse("r.md", None, ok).unwrap().warnings.is_empty());
    }

    #[test]
    fn complete_drops_reminders_that_overflow() {
        // A reminder far before due is fine on its own, but shifting it by
        // the roll-forward step must not panic; in range ones still move.
        let mut tip = Tip::parse(
            "gm.md",
            None,
            "---\ntitle: t\ndue: 2026-10-05 10:00\nremind: [2026-10-05 09:30]\nrepeat: daily at 10:00\n---\n",
        )
        .unwrap();
        tip.remind_at.push(NaiveDateTime::MAX);
        let done = tip.complete(dt("2026-10-05 11:00"));
        assert_eq!(done.due_at, Some(dt("2026-10-06 10:00")));
        assert_eq!(done.front.remind, vec!["2026-10-06 09:30"]);
    }

    #[test]
    fn display_title_prefers_paper_title_for_reading() {
        let text = "---\ntitle: read this\nkind: reading\narxiv: 2401.00001\npaper:\n  title: A Great Paper\n  authors: [A, B]\n---\n";
        let tip = Tip::parse("p.md", None, text).unwrap();
        assert_eq!(tip.display_title(), "A Great Paper");
    }

    #[test]
    fn slugify_examples() {
        assert_eq!(
            slugify("Submit ECCE camera-ready!"),
            "submit-ecce-camera-ready"
        );
        assert_eq!(slugify("   "), "tip");
        assert!(slugify(&"x".repeat(200)).len() <= 48);
    }

    #[test]
    fn slugify_keeps_unicode_letters() {
        assert_eq!(slugify("读论文 arXiv 2401"), "读论文-arxiv-2401");
        assert_eq!(slugify("Über  Größe!!"), "über-größe");
        assert_eq!(slugify("提交——终稿。"), "提交-终稿");
        let long = slugify(&"读".repeat(100));
        assert!(long.len() <= 48, "{}", long.len());
        assert_eq!(long, "读".repeat(16), "cut on a character boundary");
    }

    #[test]
    fn lenient_frontmatter_values() {
        let text = "---\ntitle: 2024\nkind: Task\npriority: HIGH\nstatus: Done\ndue: 2026-10-15\nremind: 2026-10-14\ntags: [2024, lab, true]\nlinks: https://example.org\nlocation: 302\nrepeat:\n---\nbody\n";
        let tip = Tip::parse("x.md", None, text).unwrap();
        assert_eq!(tip.front.title, "2024");
        assert_eq!(tip.front.kind, Kind::Task);
        assert_eq!(tip.front.priority, Priority::High);
        assert_eq!(tip.front.status, Status::Done);
        assert_eq!(tip.front.remind, vec!["2026-10-14"]);
        assert_eq!(tip.remind_at, vec![dt("2026-10-14 09:00")]);
        assert_eq!(tip.front.tags, vec!["2024", "lab", "true"]);
        assert_eq!(tip.front.links, vec!["https://example.org"]);
        assert_eq!(tip.front.location.as_deref(), Some("302"));
        assert_eq!(tip.front.repeat, None);
        let kinds = ["kind: Deadline", "kind: NOTE", "kind: ' reading '", "kind:"];
        let want = [Kind::Deadline, Kind::Note, Kind::Reading, Kind::Task];
        for (k, w) in kinds.iter().zip(want) {
            let t = Tip::parse("x.md", None, &format!("---\ntitle: t\n{k}\n---\n")).unwrap();
            assert_eq!(t.front.kind, w, "{k}");
        }
        assert!(Tip::parse("x.md", None, "---\ntitle: t\nkind: chore\n---\n").is_err());
        assert!(Tip::parse("x.md", None, "---\ntitle: [a, b]\n---\n").is_err());
        // Written back in canonical form: lowercase enums, lists as lists.
        let md = tip.to_markdown().unwrap();
        assert!(md.contains("kind: task"), "{md}");
        assert!(md.contains("priority: high"), "{md}");
        assert!(md.contains("status: done"), "{md}");
        assert!(md.contains("remind:\n- 2026-10-14"), "{md}");
        assert!(md.contains("title: '2024'"), "{md}");
        let again = Tip::parse("x.md", None, &md).unwrap();
        assert_eq!(again.front, tip.front);
        // The JSON shape for the UI is unchanged too.
        let json = serde_json::to_value(&tip).unwrap();
        assert_eq!(json["kind"], "task");
        assert_eq!(json["priority"], "high");
        let back: Tip = serde_json::from_value(json).unwrap();
        assert_eq!(back, tip);
    }

    #[test]
    fn highlight_round_trips_and_reads_a_single_word() {
        let text = "---
title: t
highlight: [rebuttal, Reviewer 2]
---
body
";
        let tip = Tip::parse("x.md", None, text).unwrap();
        assert_eq!(tip.front.highlight, vec!["rebuttal", "Reviewer 2"]);
        let md = tip.to_markdown().unwrap();
        assert!(
            md.contains(
                "highlight:
- rebuttal
- Reviewer 2"
            ),
            "{md}"
        );
        assert_eq!(Tip::parse("x.md", None, &md).unwrap().front, tip.front);
        let json = serde_json::to_value(&tip).unwrap();
        assert_eq!(
            json["highlight"],
            serde_json::json!(["rebuttal", "Reviewer 2"])
        );
        assert_eq!(serde_json::from_value::<Tip>(json).unwrap(), tip);
        // A scalar is a one-item list; none at all writes no key.
        let one = Tip::parse(
            "x.md",
            None,
            "---
title: t
highlight: 2024
---
",
        )
        .unwrap();
        assert_eq!(one.front.highlight, vec!["2024"]);
        let none = Tip::parse("x.md", None, SAMPLE).unwrap();
        assert!(none.front.highlight.is_empty());
        assert!(!none.to_markdown().unwrap().contains("highlight"));
        assert!(!serde_json::to_string(&none).unwrap().contains("highlight"));
    }

    #[test]
    fn unparsable_dates_become_warnings() {
        let text = "---\ntitle: t\ndue: 15/10/2026\nremind: [-1d, someday, 2026-10-14 09:00]\nsnoozed_until: later\n---\n";
        let tip = Tip::parse("x.md", None, text).unwrap();
        assert_eq!(tip.due_at, None);
        assert_eq!(tip.remind_at, vec![dt("2026-10-14 09:00")]);
        assert_eq!(
            tip.warnings,
            vec![
                r#"due: cannot parse "15/10/2026""#,
                r#"snoozed_until: cannot parse "later""#,
                r#"remind: "-1d" needs a due date"#,
                r#"remind: cannot parse "someday""#,
            ]
        );
        let huge = "---\ntitle: t\ndue: 2026-10-15\nremind: -99999999999d\n---\n";
        let tip = Tip::parse("x.md", None, huge).unwrap();
        assert_eq!(
            tip.warnings,
            vec![r#"remind: "-99999999999d" is out of range"#]
        );
        // Derived: not in the file, but carried to the UI and the cache.
        let md = tip.to_markdown().unwrap();
        assert!(!md.contains("warnings"), "{md}");
        let json = serde_json::to_string(&tip).unwrap();
        assert!(json.contains("\"warnings\""), "{json}");
        assert_eq!(serde_json::from_str::<Tip>(&json).unwrap(), tip);
        // A clean tip has none and no `warnings` key in its JSON.
        let clean = Tip::parse("x.md", None, SAMPLE).unwrap();
        assert!(clean.warnings.is_empty());
        assert!(!serde_json::to_string(&clean).unwrap().contains("warnings"));
    }
}
