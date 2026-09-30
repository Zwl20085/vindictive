//! The `Tip` domain object and its Markdown + YAML frontmatter representation.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::frontmatter::{join, split, FrontmatterError};
use super::timeparse::{format_dt, parse_due, parse_remind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Task,
    Deadline,
    Note,
    Reading,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Open,
    Done,
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
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remind: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Tile size override: `sm`, `md` or `wide`. Absent means automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Manual board position (set by drag and drop); absent means by score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arxiv: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper: Option<Paper>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Any keys we do not understand are preserved verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

/// A tip as the UI sees it: frontmatter plus resolved timestamps plus where it
/// lives in the repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "TipJson", from = "TipJson")]
pub struct Tip {
    /// Stable identifier: the file name without `.md`.
    pub id: String,
    /// Path inside the repository, e.g. `tips/2026-10-01-camera-ready.md`.
    pub path: String,
    /// Git blob SHA of the file as last fetched. Needed to update it.
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
}

/// Top-level JSON keys of a `Tip` besides the frontmatter ones.
const TIP_KEYS: [&str; 8] = [
    "id",
    "path",
    "sha",
    "body",
    "due_at",
    "remind_at",
    "snoozed_until_at",
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
        Ok(Tip::from_parts(path, sha, front, body.to_string()))
    }

    pub fn from_parts(path: &str, sha: Option<String>, front: FrontMatter, body: String) -> Tip {
        let id = id_from_path(path);
        let due_at = front.due.as_deref().and_then(parse_due);
        let mut remind_at: Vec<NaiveDateTime> = front
            .remind
            .iter()
            .filter_map(|s| parse_remind(s, due_at))
            .collect();
        remind_at.sort();
        remind_at.dedup();
        let snoozed_until_at = front.snoozed_until.as_deref().and_then(parse_due);
        Tip {
            id,
            path: path.to_string(),
            sha,
            front,
            body,
            due_at,
            remind_at,
            snoozed_until_at,
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
                    .filter_map(|r| shift.map(|s| format_dt(*r + s)))
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

/// Turn a title into a safe, short file stem.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in title.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 48 {
            break;
        }
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
        assert_eq!(slugify("读论文 arXiv 2401"), "arxiv-2401");
        assert!(slugify(&"x".repeat(200)).len() <= 48);
    }
}
