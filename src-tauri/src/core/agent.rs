//! Agent tips, protocol v1 (`docs/AGENT-PROTOCOL.md`): the `agent`
//! frontmatter map, the body hash (section 4) and the derived display state
//! (section 5). The board only reads the map; the runner is its only writer.

use std::path::{Component, Path};

use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};

/// Highest protocol version the board understands.
pub const PROTOCOL_VERSION: u64 = 1;
/// Prefix of `agent.hash`.
pub const HASH_PREFIX: &str = "sha256:";
/// Folder under the tips folder where the runner puts results (section 7).
pub const RESULTS_DIR: &str = "agent";
/// The sub-keys the board reads (protocol section 2). Everything else in the
/// map is carried, never interpreted.
pub const VIEW_KEYS: [&str; 7] = ["v", "state", "hash", "host", "run", "at", "results"];

/// The `agent` frontmatter value exactly as parsed: any YAML value, mapping
/// keys in document order, nulls, empty values and unknown sub-keys
/// included. It is written back from this value, so every board write keeps
/// it as the runner wrote it (quoting style aside). The fields the board
/// needs are read through a lenient view; nothing here is ever changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Agent(Value);

/// `deserialize_with` for `FrontMatter::agent`: a present key always yields
/// `Some`, even `agent: null`, so the board writes back what it read.
pub fn deserialize_present<'de, D>(deserializer: D) -> Result<Option<Agent>, D::Error>
where
    D: Deserializer<'de>,
{
    Agent::deserialize(deserializer).map(Some)
}

/// What the board shows for an agent tip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Queued,
    Running,
    Done,
    Question,
    Failed,
}

/// The three sub-keys the display state depends on, read leniently.
#[derive(Debug, Default, PartialEq)]
struct View<'a> {
    /// `agent.v` is present and above 1.
    newer_version: bool,
    state: Option<&'a str>,
    hash: Option<&'a str>,
}

impl Agent {
    pub fn new(value: Value) -> Self {
        Agent(value)
    }

    /// The value as parsed.
    pub fn value(&self) -> &Value {
        &self.0
    }

    fn map(&self) -> Option<&Mapping> {
        self.0.as_mapping()
    }

    fn view(&self) -> View<'_> {
        let Some(map) = self.map() else {
            return View::default();
        };
        View {
            newer_version: map.get("v").is_some_and(above_version),
            state: map.get("state").and_then(Value::as_str),
            hash: map.get("hash").and_then(Value::as_str),
        }
    }

    /// A string sub-key; `None` when absent, not a string, or the value is
    /// not a map at all.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.map()?.get(key).and_then(Value::as_str)
    }

    /// The read-only view the UI gets as JSON: the `VIEW_KEYS` whose values
    /// are text, finite numbers or booleans. `None` unless the value is a map.
    /// Always representable as JSON, whatever the map holds.
    pub fn json_view(&self) -> Option<serde_json::Value> {
        let map = self.map()?;
        let view = VIEW_KEYS
            .iter()
            .filter_map(|&key| Some((key.to_string(), json_scalar(map.get(key)?)?)))
            .collect();
        Some(serde_json::Value::Object(view))
    }

    /// The value as YAML text, for the JSON cache.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.0)
    }

    /// The value back from `to_yaml` text.
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text).map(Agent)
    }
}

/// A YAML scalar as JSON, if JSON can hold it.
fn json_scalar(value: &Value) -> Option<serde_json::Value> {
    match value {
        Value::String(s) => Some(serde_json::Value::String(s.clone())),
        Value::Bool(b) => Some(serde_json::Value::Bool(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i.into())
            } else if let Some(u) = n.as_u64() {
                Some(u.into())
            } else {
                n.as_f64()
                    .and_then(serde_json::Number::from_f64)
                    .map(serde_json::Value::Number)
            }
        }
        _ => None,
    }
}

/// A number above the protocol version (`2`, `2.0`); other values are ignored.
fn above_version(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.as_f64().is_some_and(|f| f > PROTOCOL_VERSION as f64),
        _ => false,
    }
}

/// Protocol section 4 `normalise`: no BOM, `\n` line ends, no trailing spaces
/// or tabs, no leading or trailing empty lines, no final newline.
pub fn normalise(body: &str) -> String {
    let body = body.strip_prefix('\u{feff}').unwrap_or(body);
    let unified = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = unified
        .split('\n')
        .map(|l| l.trim_end_matches([' ', '\t']))
        .collect();
    let first = lines.iter().position(|l| !l.is_empty());
    let last = lines.iter().rposition(|l| !l.is_empty());
    match (first, last) {
        (Some(a), Some(b)) => lines[a..=b].join("\n"),
        _ => String::new(),
    }
}

/// `"sha256:" + lowercase hex SHA-256 of normalise(body)`.
pub fn body_hash(body: &str) -> String {
    let digest = Sha256::digest(normalise(body).as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("{HASH_PREFIX}{hex}")
}

/// Protocol section 5: the display state, first matching row wins.
pub fn display_state(is_agent_kind: bool, agent: Option<&Agent>, body: &str) -> Option<AgentState> {
    if !is_agent_kind {
        return None;
    }
    let view = agent.map(Agent::view).unwrap_or_default();
    if view.newer_version {
        return None;
    }
    if view.state == Some("running") {
        return Some(AgentState::Running);
    }
    let current = view.hash.is_some_and(|h| h == body_hash(body));
    if view.state.is_none() || !current {
        return Some(AgentState::Queued);
    }
    Some(match view.state {
        Some("done") => AgentState::Done,
        Some("question") => AgentState::Question,
        Some("failed") => AgentState::Failed,
        _ => AgentState::Queued,
    })
}

/// Check `agent.results` before it is opened: relative to the tips folder,
/// no `..` or `.`, no drive, root or UNC prefix, no `:` anywhere (drive
/// letters and NTFS streams), and it must name something under `agent/`.
/// Returns the cleaned `/`-separated path; the caller still resolves it
/// inside the folder and requires an existing folder (never a file).
pub fn results_rel(results: &str) -> Result<String, String> {
    let cleaned = results.trim().replace('\\', "/");
    let refuse =
        || format!("refusing results path {results:?}: it must stay inside the tips folder");
    if cleaned.is_empty() {
        return Err("the agent has not recorded a results folder".into());
    }
    if cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(refuse());
    }
    let parts: Vec<&str> = cleaned.split('/').filter(|p| !p.is_empty()).collect();
    let all_normal = parts.iter().all(|p| {
        let mut comps = Path::new(p).components();
        matches!(
            (comps.next(), comps.next()),
            (Some(Component::Normal(_)), None)
        )
    });
    if parts.is_empty() || !all_normal {
        return Err(refuse());
    }
    // Only the runner's own results area: `agent/<something>`.
    if parts.len() < 2 || parts[0] != RESULTS_DIR {
        return Err(format!(
            "refusing results path {results:?}: it must be a folder under {RESULTS_DIR}/"
        ));
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(body: &str) -> String {
        body_hash(body).trim_start_matches(HASH_PREFIX).to_string()
    }

    #[test]
    fn hash_test_vectors() {
        assert_eq!(
            hex("Find three recent papers on hairpin AC loss.\r\n\r\n"),
            "a7b5da43b6fbfa12a04b1d93b1c7eb5b224f672554632f78c5bf68c6f5a2386f"
        );
        assert_eq!(
            hex("\u{feff}\n  line one  \r\n\tline two\t\n\n\n"),
            "7ceac30c0680c7b43ef2cc5971ddff12cd7584a56cf6fa615f865ffb6f97adca"
        );
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(hex(""), empty);
        assert_eq!(hex("\n \n"), empty);
        assert_eq!(
            hex("调研 ⭐\n"),
            "2115d91d771e0eafc845500cc950c60545e93cd145b2f4b80434fd36a6bf9f95"
        );
        assert_eq!(
            hex("Prompt\n\n<!-- agent:begin 2026-11-09T07:31:00 question -->\n## ❓ Questions\n1. Which year range?\n<!-- agent:end -->\n\n2020 onwards\n"),
            "262828d169635feb78f04a5a63dde71f681a21c534598a9b19556af76446b9ac"
        );
    }

    #[test]
    fn normalise_steps() {
        assert_eq!(normalise("\u{feff}a\rb\r\n c \t\n"), "a\nb\n c");
        // Inner empty lines and leading indentation stay.
        assert_eq!(normalise("\n\n  x\n\n\ny  \n\n"), "  x\n\n\ny");
        assert!(body_hash("x").starts_with("sha256:"));
    }

    fn typed(state: Option<&str>, hash: Option<String>) -> Agent {
        let mut map = Mapping::new();
        map.insert("v".into(), 1.into());
        if let Some(s) = state {
            map.insert("state".into(), s.into());
        }
        if let Some(h) = hash {
            map.insert("hash".into(), h.into());
        }
        Agent::new(Value::Mapping(map))
    }

    const BODY: &str = "Survey hairpin AC loss.\n";

    #[test]
    fn display_state_table() {
        let h = || Some(body_hash(BODY));
        let st = |a: &Agent| display_state(true, Some(a), BODY);
        // kind is not agent
        assert_eq!(
            display_state(false, Some(&typed(Some("done"), h())), BODY),
            None
        );
        // v above 1
        let newer: Agent = serde_yaml::from_str(&format!(
            "{{v: 2, state: done, hash: '{}'}}",
            body_hash(BODY)
        ))
        .unwrap();
        assert_eq!(st(&newer), None);
        // running wins over a stale or missing hash
        assert_eq!(st(&typed(Some("running"), None)), Some(AgentState::Running));
        assert_eq!(
            st(&typed(Some("running"), Some("sha256:old".into()))),
            Some(AgentState::Running)
        );
        // state absent, hash absent, hash differs
        assert_eq!(display_state(true, None, BODY), Some(AgentState::Queued));
        assert_eq!(st(&typed(None, h())), Some(AgentState::Queued));
        assert_eq!(st(&typed(Some("done"), None)), Some(AgentState::Queued));
        assert_eq!(
            st(&typed(Some("done"), Some("sha256:old".into()))),
            Some(AgentState::Queued)
        );
        // done / question / failed with a matching hash
        assert_eq!(st(&typed(Some("done"), h())), Some(AgentState::Done));
        assert_eq!(
            st(&typed(Some("question"), h())),
            Some(AgentState::Question)
        );
        assert_eq!(st(&typed(Some("failed"), h())), Some(AgentState::Failed));
        // anything else
        assert_eq!(st(&typed(Some("paused"), h())), Some(AgentState::Queued));
    }

    #[test]
    fn done_then_body_edited_is_queued() {
        let agent = typed(Some("done"), Some(body_hash(BODY)));
        assert_eq!(
            display_state(true, Some(&agent), BODY),
            Some(AgentState::Done)
        );
        // Whitespace-only changes do not count as an edit.
        let same = "\r\nSurvey hairpin AC loss.   \r\n\r\n";
        assert_eq!(
            display_state(true, Some(&agent), same),
            Some(AgentState::Done)
        );
        let edited = "Survey hairpin AC loss.\n\nOnly since 2020.\n";
        assert_eq!(
            display_state(true, Some(&agent), edited),
            Some(AgentState::Queued)
        );
    }

    #[test]
    fn raw_values_are_read_leniently() {
        let raw = |yaml: &str| -> Agent { serde_yaml::from_str(yaml).unwrap() };
        let h = body_hash(BODY);
        // Wrong type on a field the state does not need: still Done.
        let odd = raw(&format!("{{v: 1, state: done, hash: '{h}', host: 42}}"));
        assert_eq!(
            display_state(true, Some(&odd), BODY),
            Some(AgentState::Done)
        );
        // A newer float version hides the state.
        let newer = raw(&format!("{{v: 2.0, state: done, hash: '{h}'}}"));
        assert_eq!(display_state(true, Some(&newer), BODY), None);
        // Scalars, lists and null behave like an absent map.
        for yaml in ["yes", "[1, 2]", "null", "'text'"] {
            assert_eq!(
                display_state(true, Some(&raw(yaml)), BODY),
                Some(AgentState::Queued)
            );
        }
        assert_eq!(odd.text("state"), Some("done"));
        assert_eq!(odd.text("host"), None);
    }

    #[test]
    fn only_a_yaml_number_above_one_is_a_newer_version() {
        let h = body_hash(BODY);
        let state_for = |v: &str| {
            let text = format!("v: {v}\nstate: done\nhash: '{h}'\n");
            let agent: Agent = serde_yaml::from_str(&text).unwrap();
            display_state(true, Some(&agent), BODY)
        };
        // Numbers above 1, integer or float, hide the state.
        assert_eq!(state_for("2"), None);
        assert_eq!(state_for("2.0"), None);
        assert_eq!(state_for("1.5"), None);
        // Strings never do, even when they look like a number.
        assert_eq!(state_for("\"2\""), Some(AgentState::Done));
        assert_eq!(state_for("'2.0'"), Some(AgentState::Done));
        assert_eq!(state_for("two"), Some(AgentState::Done));
        // 1, 1.0, 0 and negatives are version 1 or below.
        for v in ["1", "1.0", "0", "-3"] {
            assert_eq!(state_for(v), Some(AgentState::Done), "v: {v}");
        }
    }

    #[test]
    fn the_view_reads_known_keys_and_carries_the_rest() {
        let a: Agent = serde_yaml::from_str(
            "zeta: [1, 2]\nv: 1\nstate: question\nhash: sha256:ab\nhost: DESK\nrun: 20261109-073100-ab12\nat: 2026-11-09T07:31:00\nresults: agent/x/\n",
        )
        .unwrap();
        assert_eq!(a.text("at"), Some("2026-11-09T07:31:00"));
        assert_eq!(a.text("results"), Some("agent/x/"));
        assert_eq!(a.text("run"), Some("20261109-073100-ab12"));
        // The value itself is untouched: same keys, same order.
        let keys: Vec<&str> = a
            .value()
            .as_mapping()
            .unwrap()
            .keys()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            keys,
            ["zeta", "v", "state", "hash", "host", "run", "at", "results"]
        );
        // The JSON view has only the known keys, and only JSON-safe values.
        let view = a.json_view().unwrap();
        assert_eq!(view["v"], 1);
        assert_eq!(view["host"], "DESK");
        assert!(view.get("zeta").is_none());
        let odd: Agent = serde_yaml::from_str("{v: .nan, host: [x], 1: y, run: true}").unwrap();
        assert_eq!(odd.json_view().unwrap(), serde_json::json!({ "run": true }));
        let list: Agent = serde_yaml::from_str("[a, b]").unwrap();
        assert_eq!(list.json_view(), None);
        // YAML text round trip, as the cache stores it.
        let back = Agent::from_yaml(&a.to_yaml().unwrap()).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn results_paths_are_checked() {
        assert_eq!(results_rel("agent/survey/").unwrap(), "agent/survey");
        assert_eq!(
            results_rel(" agent\\survey\\deep ").unwrap(),
            "agent/survey/deep"
        );
        assert_eq!(results_rel("agent//x").unwrap(), "agent/x");
        // Outside the runner's `agent/` area.
        for outside in [
            "agent",
            "agent/",
            "survey/",
            "figures/x",
            "Agent/x",
            "agents/x",
            "x/agent/y",
        ] {
            let err = results_rel(outside).unwrap_err();
            assert!(err.contains("under agent/"), "{outside:?}: {err}");
        }
        for bad in [
            "",
            "   ",
            "/",
            "/etc/passwd",
            "\\\\server\\share",
            "C:\\Windows",
            "C:agent",
            "agent/../../x",
            "..",
            "./agent",
            "agent/./x",
            "agent/x:stream",
        ] {
            assert!(results_rel(bad).is_err(), "{bad:?} must be refused");
        }
    }
}
