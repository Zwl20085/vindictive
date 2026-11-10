//! The `Tip` domain object and its Markdown + YAML frontmatter representation.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::agent::{self, Agent, AgentState};
use super::frontmatter::{join, split, FrontmatterError};
use super::question::{id_for_path, is_question_path};
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
    /// A task for an AI agent, see `docs/AGENT-PROTOCOL.md`.
    Agent,
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
    "agent" => Agent,
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
    /// Agent run state, written only by the runner (`docs/AGENT-PROTOCOL.md`).
    /// The board never changes it; any value, even a malformed one, is kept.
    #[serde(
        default,
        deserialize_with = "agent::deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent: Option<Agent>,
    /// Any keys we do not understand are preserved verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

/// A tip as the UI sees it: frontmatter plus resolved timestamps plus where it
/// lives in the tips folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "TipJson", from = "TipJson")]
pub struct Tip {
    /// Stable identifier: the file name without `.md`; for a question tip
    /// (`questions/<name>.md`) `questions/<name>`, see `core::question`.
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
    /// Derived agent display state (protocol section 5); never written to the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_state: Option<AgentState>,
}

/// Top-level JSON keys of a `Tip` besides the frontmatter ones.
const TIP_KEYS: [&str; 11] = [
    "id",
    "path",
    "sha",
    "body",
    "due_at",
    "remind_at",
    "snoozed_until_at",
    "warnings",
    "agent_state",
    "agent_yaml",
    "shadowed",
];

/// JSON shape of a `Tip` (IPC and the offline cache). The frontmatter is
/// flattened into it, so an unknown frontmatter key named like a tip field
/// (`id: 20240101` from Obsidian, say) would emit that key twice. Such keys
/// travel in `shadowed` instead and are put back on the way in.
///
/// The `agent` map does not travel as JSON (it may hold keys JSON cannot,
/// like `1:` or `~:`, or `.nan`): `agent` is only the read-only view the UI
/// shows, and `agent_yaml` is the value as YAML text, read back on the way in.
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
    /// Output only: recomputed from the frontmatter and body on the way in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent_state: Option<AgentState>,
    /// The `agent` view for the UI (`Agent::json_view`). Ignored on the way
    /// in, except in a cache written before `agent_yaml` existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<serde_json::Value>,
    /// The `agent` value as YAML text; empty if it could not be written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent_yaml: Option<String>,
}

impl From<Tip> for TipJson {
    fn from(tip: Tip) -> Self {
        let mut front = tip.front;
        let (shadowed, extra) = std::mem::take(&mut front.extra)
            .into_iter()
            .partition(|(k, _)| TIP_KEYS.contains(&k.as_str()));
        front.extra = extra;
        let agent = front.agent.take();
        let view = agent.as_ref().and_then(Agent::json_view);
        let agent_yaml = agent.map(|a| a.to_yaml().unwrap_or_default());
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
            agent_state: tip.agent_state,
            agent: view,
            agent_yaml,
        }
    }
}

impl From<TipJson> for Tip {
    fn from(json: TipJson) -> Self {
        let mut front = json.front;
        front.extra.extend(json.shadowed);
        let (agent, exact) = agent_from_json(json.agent_yaml.as_deref(), json.agent);
        front.agent = agent;
        let agent_state = agent_state_of(&front, &json.body, &json.path);
        Tip {
            id: json.id,
            path: json.path,
            // A copy that may differ from the file is not written back: with
            // no stamp, the next scan reads the file again first.
            sha: json.sha.filter(|_| exact),
            front,
            body: json.body,
            due_at: json.due_at,
            remind_at: json.remind_at,
            snoozed_until_at: json.snoozed_until_at,
            warnings: json.warnings,
            agent_state,
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
        let id = id_for_path(path);
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
        let agent_state = agent_state_of(&front, &body, path);
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
            agent_state,
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

    /// A question tip: its file is in the `questions/` subfolder. Derived
    /// from the path, never stored.
    pub fn is_question(&self) -> bool {
        is_question_path(&self.path)
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

    /// The tip with fetched paper metadata, as `enrich` saves it: a plain
    /// task becomes a reading tip; every other kind is kept.
    pub fn enriched(&self, paper: Paper) -> Tip {
        let mut front = self.front.clone();
        front.paper = Some(paper);
        if front.kind == Kind::Task {
            front.kind = Kind::Reading;
        }
        self.with_front(front)
    }
}

/// The `agent` value of a JSON tip, and whether it is exactly the file's:
/// from `agent_yaml` when present; else, in a cache written before it
/// existed, from the JSON map (key order and non-string keys may be lost).
fn agent_from_json(yaml: Option<&str>, old: Option<serde_json::Value>) -> (Option<Agent>, bool) {
    match (yaml, old) {
        (Some(text), _) if !text.trim().is_empty() => match Agent::from_yaml(text) {
            Ok(agent) => (Some(agent), true),
            Err(_) => (None, false),
        },
        (Some(_), _) => (None, false),
        (None, Some(json)) => (serde_yaml::to_value(json).ok().map(Agent::new), false),
        (None, None) => (None, true),
    }
}

/// The agent badge state. A question tip never has one, whatever its kind:
/// the runner only scans the top level of the tips folder.
fn agent_state_of(front: &FrontMatter, body: &str, path: &str) -> Option<AgentState> {
    if is_question_path(path) {
        return None;
    }
    agent::display_state(front.kind == Kind::Agent, front.agent.as_ref(), body)
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
    use crate::core::frontmatter::split;

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

    const AGENT_BLOCK: &str = "agent:
  v: 1
  state: question
  hash: sha256:262828d169635feb78f04a5a63dde71f681a21c534598a9b19556af76446b9ac
  host: DESKTOP-ABC
  run: 20261109-073100-ab12
  at: 2026-11-09T07:31:00
  results: agent/survey-hairpin-ac-loss-papers/
  zz_future:
    nested:
    - 1
    - 2
  zz_note: keep me
";
    const AGENT_BODY: &str = "Prompt

<!-- agent:begin 2026-11-09T07:31:00 question -->
## ❓ Questions
1. Which year range?
<!-- agent:end -->

2020 onwards
";

    fn agent_file(agent_yaml: &str) -> String {
        format!(
            "---
title: Survey
kind: agent
status: open
due: 2026-11-20
{agent_yaml}---

{AGENT_BODY}"
        )
    }

    /// The full Edit form as the frontend fills it for `tip`: every field
    /// shows the file's value. There is no `agent` field.
    fn form_of(tip: &Tip) -> super::super::edit::TipEdit {
        let f = &tip.front;
        super::super::edit::TipEdit {
            title: f.title.clone(),
            kind: f.kind,
            priority: f.priority,
            due: f.due.clone(),
            location: f.location.clone(),
            tags: f.tags.clone(),
            remind: f.remind.clone(),
            repeat: f.repeat.clone(),
            links: f.links.clone(),
            images: f.images.clone(),
            color: f.color.clone(),
            size: f.size.clone(),
            highlight: f.highlight.clone(),
            body: tip.body.clone(),
        }
    }

    /// Every way the board rewrites an existing tip, as the commands build
    /// them (none of them touches the body):
    /// `update_tip` (the full Edit form, saved unchanged and with every field
    /// changed), `mark_done` (plain and recurring), `reopen`, `snooze`,
    /// `set_color`, `set_size`, `set_order` and `enrich_tip`.
    fn board_writes(tip: &Tip) -> Vec<(&'static str, Tip)> {
        use super::super::edit::{apply, TipEdit};
        let now = dt("2026-11-09 08:00");
        let all_changed = TipEdit {
            title: "Survey, renamed".into(),
            kind: Kind::Agent,
            priority: Priority::High,
            due: Some("2026-11-21 17:00".into()),
            location: Some("Lab 302".into()),
            tags: vec!["lit".into()],
            remind: vec!["-1d".into()],
            repeat: Some("weekly on mon".into()),
            links: vec!["https://example.org".into()],
            images: vec!["figures/plot.png".into()],
            color: Some("#4A3B6B".into()),
            size: Some("wide".into()),
            highlight: vec!["hairpin".into()],
            ..form_of(tip)
        };
        let with = |f: fn(&mut FrontMatter)| {
            let mut front = tip.front.clone();
            f(&mut front);
            tip.with_front(front)
        };
        let recurring = with(|f| f.repeat = Some("weekly on mon".into()));
        let paper = Paper {
            title: "A paper".into(),
            ..Default::default()
        };
        vec![
            ("update_tip unchanged", apply(tip, form_of(tip)).unwrap()),
            ("update_tip all fields", apply(tip, all_changed).unwrap()),
            ("mark_done", tip.complete(now)),
            ("mark_done recurring", recurring.complete(now)),
            ("reopen", tip.complete(now).reopen()),
            ("snooze", tip.snooze_until(now)),
            ("set_color", with(|f| f.color = Some("#4A3B6B".into()))),
            ("set_size", with(|f| f.size = Some("wide".into()))),
            ("set_order", with(|f| f.order = Some(1.5))),
            ("enrich_tip", tip.enriched(paper)),
        ]
    }

    /// `AGENT_BLOCK` as the runner leaves it after a finished run: state
    /// `done` and the hash of the current body, so the board shows ⭐.
    fn done_block() -> String {
        AGENT_BLOCK
            .replace("state: question", "state: done")
            .replace(
                "sha256:262828d169635feb78f04a5a63dde71f681a21c534598a9b19556af76446b9ac",
                &agent::body_hash(AGENT_BODY),
            )
    }

    #[test]
    fn board_writes_keep_a_finished_agent_tip_finished() {
        let block = done_block();
        let tip = Tip::parse("survey.md", Some("s1".into()), &agent_file(&block)).unwrap();
        assert_eq!(tip.agent_state, Some(AgentState::Done));
        for (name, written) in board_writes(&tip) {
            let text = written.to_markdown().unwrap();
            assert_eq!(agent_lines(&text), block, "{name}:\n{text}");
            let again = Tip::parse("survey.md", None, &text).unwrap();
            assert_eq!(again.body, tip.body, "{name}");
            // Marking it Done retires the task but does not change what
            // the badge says about the last run.
            assert_eq!(written.agent_state, Some(AgentState::Done), "{name}");
            assert_eq!(again.agent_state, Some(AgentState::Done), "{name}");
        }
    }

    #[test]
    fn a_body_edit_requeues_the_agent_tip_and_keeps_the_map() {
        use super::super::edit::{apply, TipEdit};
        let block = done_block();
        let tip = Tip::parse("survey.md", Some("s1".into()), &agent_file(&block)).unwrap();
        let answer = TipEdit {
            body: format!("{AGENT_BODY}\nAlso 2019, please.\n"),
            ..form_of(&tip)
        };
        let edited = apply(&tip, answer).unwrap();
        assert_eq!(edited.agent_state, Some(AgentState::Queued));
        let text = edited.to_markdown().unwrap();
        assert_eq!(agent_lines(&text), block, "{text}");
        let again = Tip::parse("survey.md", None, &text).unwrap();
        assert_eq!(again.front.agent, tip.front.agent);
        assert_eq!(again.agent_state, Some(AgentState::Queued));
        // Whitespace-only changes do not count as an edit (protocol section 4).
        let spaced = TipEdit {
            body: format!("\n{}  \r\n\n", AGENT_BODY.trim_end()),
            ..form_of(&tip)
        };
        assert_eq!(
            apply(&tip, spaced).unwrap().agent_state,
            Some(AgentState::Done)
        );
    }

    #[test]
    fn changing_the_kind_away_from_agent_keeps_the_map() {
        use super::super::edit::{apply, TipEdit};
        let block = done_block();
        let tip = Tip::parse("survey.md", Some("s1".into()), &agent_file(&block)).unwrap();
        let task = apply(
            &tip,
            TipEdit {
                kind: Kind::Task,
                ..form_of(&tip)
            },
        )
        .unwrap();
        // Not an agent tip any more: no display state, but the map stays
        // exactly as the runner wrote it (the board never removes it).
        assert_eq!(task.front.kind, Kind::Task);
        assert_eq!(task.agent_state, None);
        let text = task.to_markdown().unwrap();
        assert_eq!(agent_lines(&text), block, "{text}");
        // Switching back brings the old state back, since nothing was lost.
        let back = apply(
            &task,
            TipEdit {
                kind: Kind::Agent,
                ..form_of(&task)
            },
        )
        .unwrap();
        assert_eq!(back.front.agent, tip.front.agent);
        assert_eq!(back.agent_state, Some(AgentState::Done));
    }

    fn agent_lines(markdown: &str) -> String {
        let mut out = String::new();
        let mut inside = false;
        for line in markdown.lines() {
            if line == "---" && inside {
                break;
            }
            if line.starts_with("agent:") {
                inside = true;
            } else if inside && !line.starts_with(' ') && !line.starts_with('-') {
                break;
            }
            if inside {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    #[test]
    fn agent_map_survives_every_board_write_byte_for_byte() {
        let tip = Tip::parse("survey.md", Some("s1".into()), &agent_file(AGENT_BLOCK)).unwrap();
        assert_eq!(tip.front.kind, Kind::Agent);
        assert!(tip
            .front
            .agent
            .as_ref()
            .is_some_and(|a| a.value().is_mapping()));
        assert_eq!(tip.agent_state, Some(AgentState::Question));
        for (name, written) in board_writes(&tip) {
            let text = written.to_markdown().unwrap();
            assert_eq!(
                agent_lines(&text),
                AGENT_BLOCK,
                "{name}:
{text}"
            );
            let again = Tip::parse("survey.md", None, &text).unwrap();
            assert_eq!(again.front.agent, tip.front.agent, "{name}");
            assert_eq!(again.body, tip.body, "{name}");
        }
    }

    #[test]
    fn malformed_agent_values_stay_readable_and_are_kept() {
        for agent_yaml in [
            "agent: [not, a, map]
",
            "agent: just text
",
            "agent: null
",
            "agent:
  v: one
  state: done
  host: 42
",
            "agent:
  v: 1
  state: {weird: yes}
",
        ] {
            let text = agent_file(agent_yaml);
            let tip = Tip::parse("survey.md", None, &text).unwrap();
            let original: serde_yaml::Value =
                serde_yaml::from_str(split(&text).unwrap().0).unwrap();
            assert!(tip.front.agent.is_some(), "{agent_yaml}");
            assert_eq!(tip.agent_state, Some(AgentState::Queued), "{agent_yaml}");
            for (name, written) in board_writes(&tip) {
                let out = written.to_markdown().unwrap();
                let value: serde_yaml::Value =
                    serde_yaml::from_str(split(&out).unwrap().0).unwrap();
                assert_eq!(
                    value["agent"], original["agent"],
                    "{name} {agent_yaml}:
{out}"
                );
            }
        }
    }

    /// The lenient frontmatter rules and the lenient `agent` map agree: a
    /// hand-written file with loose values elsewhere, a mixed-case kind and a
    /// malformed map still loads, with warnings only for what the board
    /// itself resolves (here a bad `due`), never for the runner's map.
    #[test]
    fn lenient_frontmatter_and_a_malformed_agent_map_load_together() {
        let text = "---\ntitle: 2024\nkind: ' Agent '\npriority: HIGH\ndue: someday\ntags: lab\nagent: [not, a, map]\n---\nPrompt\n";
        let tip = Tip::parse("x.md", None, text).unwrap();
        assert_eq!(tip.front.title, "2024");
        assert_eq!(tip.front.kind, Kind::Agent);
        assert_eq!(tip.front.tags, vec!["lab"]);
        assert_eq!(tip.agent_state, Some(AgentState::Queued));
        assert_eq!(tip.warnings, vec![r#"due: cannot parse "someday""#]);
        let out = tip.to_markdown().unwrap();
        assert!(out.contains("kind: agent\n"), "{out}");
        assert!(out.contains("agent:\n- not\n- a\n- map\n"), "{out}");
    }

    /// The `agent` value of a written file, straight from its YAML.
    fn agent_value(markdown: &str) -> serde_yaml::Value {
        let front: serde_yaml::Value = serde_yaml::from_str(split(markdown).unwrap().0).unwrap();
        front
            .as_mapping()
            .and_then(|m| m.get("agent"))
            .cloned()
            .expect("the file has an agent key")
    }

    /// Every mapping key in `value`, depth first, in document order, with
    /// its path; equal `Value`s can still differ here, since mapping
    /// equality ignores order.
    fn key_order(value: &serde_yaml::Value) -> Vec<String> {
        fn walk(value: &serde_yaml::Value, path: &str, out: &mut Vec<String>) {
            match value {
                serde_yaml::Value::Mapping(map) => {
                    for (k, v) in map {
                        let key = format!("{path}/{}", serde_yaml::to_string(k).unwrap().trim());
                        out.push(key.clone());
                        walk(v, &key, out);
                    }
                }
                serde_yaml::Value::Sequence(items) => {
                    for (i, v) in items.iter().enumerate() {
                        walk(v, &format!("{path}[{i}]"), out);
                    }
                }
                serde_yaml::Value::Tagged(t) => walk(&t.value, path, out),
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(value, "", &mut out);
        out
    }

    /// A well-typed map in no canonical order, with nulls and empty values.
    const UNORDERED_AGENT: &str = "agent:
  results:
  state: done
  v: 1
  zeta: last-but-written-first
  host: null
  run: ~
  hash: sha256:x
  alpha:
    zulu: 1
    beta: [b, a]
    alpha: {y: 2, x: 1}
  at: 2026-11-09T07:31:00
";

    #[test]
    fn agent_values_keep_order_and_nulls_through_every_board_write() {
        let text = agent_file(UNORDERED_AGENT);
        let original = agent_value(&text);
        let order = key_order(&original);
        assert_eq!(
            order[..4],
            ["/results", "/state", "/v", "/zeta"],
            "the fixture is unordered"
        );
        let tip = Tip::parse("survey.md", Some("s1".into()), &text).unwrap();
        for (name, written) in board_writes(&tip) {
            let out = written.to_markdown().unwrap();
            let value = agent_value(&out);
            assert_eq!(value, original, "{name}:\n{out}");
            assert_eq!(key_order(&value), order, "{name}:\n{out}");
        }
    }

    /// Keys and values JSON cannot hold: an integer key, a null key, a
    /// sequence key, `.inf` and `.nan`.
    const JSON_HOSTILE_AGENT: &str = "agent:
  v: 1
  state: done
  1: integer key
  ~: null key
  ? [a, b]
  : sequence key
  big: .inf
  odd: .nan
  nested: {2: two, ~: none}
";

    #[test]
    fn agent_values_survive_the_json_cache_and_a_later_write() {
        let text = agent_file(JSON_HOSTILE_AGENT);
        let original = agent_value(&text);
        let tip = Tip::parse("survey.md", Some("s1".into()), &text).unwrap();
        // What `save_cache` and the board emit do: never fails.
        let json = serde_json::to_string(&vec![tip.clone()]).expect("the board serialises");
        // What `load_cache` does, then a board write from the cached copy.
        let cached: Vec<Tip> = serde_json::from_str(&json).unwrap();
        assert_eq!(cached[0], tip);
        for (name, written) in board_writes(&cached[0]) {
            let out = written.to_markdown().unwrap();
            let value = agent_value(&out);
            assert_eq!(value, original, "{name}:\n{out}");
            assert_eq!(key_order(&value), key_order(&original), "{name}:\n{out}");
        }
    }

    #[test]
    fn old_cache_entries_still_load() {
        // 0.5.3: no agent data at all.
        let plain = serde_json::json!({
            "id": "p", "path": "p.md", "sha": "s1", "title": "Plain",
            "kind": "task", "priority": "normal", "status": "open",
            "body": "x", "remind_at": [],
        });
        let tip: Tip = serde_json::from_value(plain).unwrap();
        assert_eq!(tip.sha.as_deref(), Some("s1"));
        assert!(tip.front.agent.is_none());
        // The pre-release agent build: the whole map as a JSON object. It
        // loads, and its stamp is dropped so the file is read again before
        // anything writes it (the JSON copy may have lost key order).
        let old = serde_json::json!({
            "id": "a", "path": "a.md", "sha": "s2", "title": "Agent",
            "kind": "agent", "priority": "normal", "status": "open",
            "agent": { "v": 1, "state": "done", "hash": "sha256:x", "zz": "keep" },
            "agent_state": "done", "body": "x", "remind_at": [],
        });
        let tip: Tip = serde_json::from_value(old).unwrap();
        assert_eq!(tip.front.kind, Kind::Agent);
        assert!(tip.front.agent.is_some());
        assert_eq!(tip.agent_state, Some(AgentState::Queued));
        assert_eq!(tip.sha, None);
    }

    #[test]
    fn agent_fields_travel_through_json() {
        let tip = Tip::parse("survey.md", Some("s1".into()), &agent_file(AGENT_BLOCK)).unwrap();
        let json = serde_json::to_value(&tip).unwrap();
        assert_eq!(json["kind"], "agent");
        assert_eq!(json["agent_state"], "question");
        assert_eq!(json["agent"]["v"], 1);
        assert_eq!(json["agent"]["host"], "DESKTOP-ABC");
        assert_eq!(
            json["agent"]["results"],
            "agent/survey-hairpin-ac-loss-papers/"
        );
        // The UI gets the view only; the whole map travels as YAML text.
        assert!(json["agent"].get("zz_future").is_none());
        let yaml = json["agent_yaml"].as_str().unwrap();
        assert!(
            yaml.contains("zz_future:") && yaml.contains("zz_note: keep me"),
            "{yaml}"
        );
        let back: Tip = serde_json::from_value(json).unwrap();
        assert_eq!(back, tip);
        assert_eq!(back.sha.as_deref(), Some("s1"));
        // A stale `agent_state` in the JSON is ignored: it is always derived.
        let mut stale = serde_json::to_value(&tip).unwrap();
        stale["agent_state"] = "done".into();
        let back: Tip = serde_json::from_value(stale).unwrap();
        assert_eq!(back.agent_state, Some(AgentState::Question));
        // Non-agent tips carry none of the fields.
        let plain = serde_json::to_value(Tip::parse("p.md", None, SAMPLE).unwrap()).unwrap();
        assert!(plain.get("agent").is_none() && plain.get("agent_state").is_none());
        assert!(plain.get("agent_yaml").is_none());
        // A cached `agent_yaml` that does not parse: no map, and no stamp,
        // so the file is read again before anything writes it.
        let mut broken = serde_json::to_value(&tip).unwrap();
        broken["agent_yaml"] = "v: [unclosed".into();
        let back: Tip = serde_json::from_value(broken).unwrap();
        assert!(back.front.agent.is_none());
        assert_eq!(back.sha, None);
    }

    #[test]
    fn a_frontmatter_key_named_agent_state_is_shadowed_not_lost() {
        let text = "---
title: T
kind: agent
agent_state: done
---
body
";
        let tip = Tip::parse("t.md", None, text).unwrap();
        assert_eq!(tip.agent_state, Some(AgentState::Queued));
        let json = serde_json::to_value(&tip).unwrap();
        assert_eq!(json["agent_state"], "queued");
        assert_eq!(json["shadowed"]["agent_state"], "done");
        let back: Tip = serde_json::from_value(json).unwrap();
        assert!(back.to_markdown().unwrap().contains("agent_state: done"));
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

    #[test]
    fn question_tips_are_identified_by_their_folder() {
        let main = Tip::parse("survey.md", None, SAMPLE).unwrap();
        let question = Tip::parse("questions/survey.md", None, SAMPLE).unwrap();
        assert_eq!(main.id, "survey");
        assert_eq!(question.id, "questions/survey");
        assert!(!main.is_question());
        assert!(question.is_question());
        // Derived from the path only: nothing is written into the file.
        let md = question.to_markdown().unwrap();
        assert!(!md.contains("question"), "{md}");
        // No new JSON key either, so caches stay what older boards expect.
        let json = serde_json::to_value(&question).unwrap();
        assert_eq!(json["id"], "questions/survey");
        assert_eq!(json["path"], "questions/survey.md");
        assert!(json.get("question").is_none(), "{json}");
    }

    #[test]
    fn a_question_tip_is_never_an_agent_task() {
        let text = agent_file(AGENT_BLOCK);
        let main = Tip::parse("survey.md", Some("s1".into()), &text).unwrap();
        assert_eq!(main.agent_state, Some(AgentState::Question));
        let question = Tip::parse("questions/survey.md", Some("s1".into()), &text).unwrap();
        assert_eq!(question.front.kind, Kind::Agent, "the file keeps its kind");
        assert!(question.front.agent.is_some(), "and its agent map");
        assert_eq!(question.agent_state, None);
        // Through the cache, and through every board write, it stays so.
        let json = serde_json::to_string(&question).unwrap();
        assert!(!json.contains("agent_state"), "{json}");
        let cached: Tip = serde_json::from_str(&json).unwrap();
        assert_eq!(cached, question);
        for (name, written) in board_writes(&question) {
            assert_eq!(written.agent_state, None, "{name}");
            assert_eq!(written.path, "questions/survey.md", "{name}");
            assert_eq!(written.id, "questions/survey", "{name}");
        }
    }
}
