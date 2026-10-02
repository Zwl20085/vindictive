//! In-app editing of a tip: validate the fields the edit form sends and
//! build the new tip. Every frontmatter key the form does not show (remind,
//! links, images, repeat, colour, size, order, paper ids, snooze, done,
//! created and unknown keys) is carried over untouched.

use serde::Deserialize;

use super::timeparse::parse_due;
use super::tip::{FrontMatter, Kind, Priority, Tip};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_LOCATION_CHARS: usize = 200;
pub const MAX_TAGS: usize = 32;
pub const MAX_TAG_CHARS: usize = 64;
pub const MAX_BODY_CHARS: usize = 100_000;
pub const MAX_DUE_CHARS: usize = 40;

/// The fields of the edit form, as the frontend sends them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TipEdit {
    pub title: String,
    pub kind: Kind,
    pub priority: Priority,
    /// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`; `None` or blank clears it.
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub body: String,
}

/// Validate `edit` and return the edited copy of `tip`. The file path, and
/// so the id, never changes: renaming the title keeps the file name.
pub fn apply(tip: &Tip, edit: TipEdit) -> Result<Tip, String> {
    let title = edit.title.trim().to_string();
    if title.is_empty() {
        return Err("a tip needs a title".into());
    }
    check_len("title", &title, MAX_TITLE_CHARS)?;
    let location = optional(edit.location.as_deref());
    if let Some(l) = &location {
        check_len("location", l, MAX_LOCATION_CHARS)?;
    }
    let body = edit.body.replace("\r\n", "\n");
    check_len("text", &body, MAX_BODY_CHARS)?;
    let front = FrontMatter {
        title,
        kind: edit.kind,
        priority: edit.priority,
        due: due(tip, edit.due.as_deref())?,
        location,
        tags: tags(&edit.tags)?,
        ..tip.front.clone()
    };
    Ok(Tip::from_parts(&tip.path, tip.sha.clone(), front, body))
}

fn check_len(what: &str, value: &str, max: usize) -> Result<(), String> {
    if value.chars().count() > max {
        return Err(format!("the {what} is too long (at most {max} characters)"));
    }
    Ok(())
}

fn optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// The new `due` string. Unchanged dates keep the string as written in the
/// file (`2026/10/15`, a bare date, a time zone...), so a save that did not
/// touch the date does not rewrite it.
fn due(tip: &Tip, value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = optional(value) else {
        return Ok(None);
    };
    check_len("due date", &value, MAX_DUE_CHARS)?;
    let parsed = parse_due(&value).ok_or_else(|| {
        format!("{value:?} is not a date; use YYYY-MM-DD, optionally followed by HH:MM")
    })?;
    let same_as_before = tip.due_at == Some(parsed) && tip.front.due.is_some();
    let keeps_shape = tip.front.due.as_deref().map(has_time) == Some(has_time(&value));
    if same_as_before && keeps_shape {
        return Ok(tip.front.due.clone());
    }
    Ok(Some(value))
}

/// True when a date string carries a time of day (`... 09:00`).
fn has_time(value: &str) -> bool {
    value.contains(':')
}

/// Tags without `#`, empty entries dropped, duplicates removed in order.
fn tags(raw: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for tag in raw {
        let tag = tag.trim().trim_start_matches(['#', '＃']).trim();
        if tag.is_empty() || out.iter().any(|t| t == tag) {
            continue;
        }
        if tag.chars().any(char::is_whitespace) {
            return Err(format!("tag {tag:?} cannot contain spaces"));
        }
        check_len("tag", tag, MAX_TAG_CHARS)?;
        out.push(tag.to_string());
    }
    if out.len() > MAX_TAGS {
        return Err(format!("at most {MAX_TAGS} tags"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    const FILE: &str = "---\ntitle: Submit camera-ready\nkind: deadline\npriority: high\ndue: 2026-10-15\nremind:\n  - -1d\nlocation: Lab 302\nlinks:\n  - https://example.org\nimages: [fig.png]\ntags: [ecce, paper]\nrepeat: weekly on mon\ncolor: '#4A3B6B'\nsize: wide\norder: 2.5\narxiv: 2401.00001\nsnoozed_until: 2026-10-01 12:00\ndone_at: 2026-09-30 10:00\ncreated: 2026-09-01 09:00\ncustom_key: keep me\n---\n\nOld body.\n";

    fn tip() -> Tip {
        Tip::parse("tips/2026-camera.md", Some("stamp-1".into()), FILE).unwrap()
    }

    fn edit() -> TipEdit {
        TipEdit {
            title: "Submit camera-ready".into(),
            kind: Kind::Deadline,
            priority: Priority::High,
            due: Some("2026-10-15".into()),
            location: Some("Lab 302".into()),
            tags: vec!["ecce".into(), "paper".into()],
            body: "\nOld body.\n".into(),
        }
    }

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn unchanged_form_round_trips_the_tip() {
        let original = tip();
        let same = apply(&original, edit()).unwrap();
        assert_eq!(same, original);
    }

    #[test]
    fn edits_the_form_fields_and_keeps_everything_else() {
        let original = tip();
        let edited = apply(
            &original,
            TipEdit {
                title: "  Camera-ready v2 ".into(),
                kind: Kind::Task,
                priority: Priority::Low,
                due: Some("2026-10-20 17:00".into()),
                location: None,
                tags: vec!["#ecce".into(), "final".into(), "ecce".into(), " ".into()],
                body: "New body\r\nline 2".into(),
            },
        )
        .unwrap();
        assert_eq!(edited.front.title, "Camera-ready v2");
        assert_eq!(edited.front.kind, Kind::Task);
        assert_eq!(edited.front.priority, Priority::Low);
        assert_eq!(edited.front.due.as_deref(), Some("2026-10-20 17:00"));
        assert_eq!(edited.due_at, Some(dt("2026-10-20 17:00")));
        // Relative reminders follow the new due date.
        assert_eq!(edited.remind_at, vec![dt("2026-10-19 17:00")]);
        assert_eq!(edited.front.location, None);
        assert_eq!(edited.front.tags, vec!["ecce", "final"]);
        assert_eq!(edited.body, "New body\nline 2");
        // Identity and untouched keys.
        assert_eq!(edited.id, original.id);
        assert_eq!(edited.path, original.path);
        assert_eq!(edited.sha, original.sha);
        let (a, b) = (&edited.front, &original.front);
        assert_eq!(a.remind, b.remind);
        assert_eq!(a.links, b.links);
        assert_eq!(a.images, b.images);
        assert_eq!(a.repeat, b.repeat);
        assert_eq!(a.color, b.color);
        assert_eq!(a.size, b.size);
        assert_eq!(a.order, b.order);
        assert_eq!(a.arxiv, b.arxiv);
        assert_eq!(a.snoozed_until, b.snoozed_until);
        assert_eq!(a.done_at, b.done_at);
        assert_eq!(a.created, b.created);
        assert_eq!(a.status, b.status);
        assert_eq!(a.extra, b.extra);
        // The original is not mutated.
        assert_eq!(original.front.title, "Submit camera-ready");
        // And it survives a write and a read.
        let text = edited.to_markdown().unwrap();
        assert!(text.contains("custom_key: keep me"), "{text}");
        let again = Tip::parse(&edited.path, None, &text).unwrap();
        assert_eq!(again.front, edited.front);
    }

    #[test]
    fn a_bare_date_stays_a_bare_date() {
        let edited = apply(
            &tip(),
            TipEdit {
                due: Some("2026-10-18".into()),
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(edited.front.due.as_deref(), Some("2026-10-18"));
        assert_eq!(edited.due_at, Some(dt("2026-10-18 23:59")));
        // Adding a time to the same day is a change, not "unchanged".
        let timed = apply(
            &tip(),
            TipEdit {
                due: Some("2026-10-15 23:59".into()),
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(timed.front.due.as_deref(), Some("2026-10-15 23:59"));
    }

    #[test]
    fn hand_written_due_formats_survive_an_unrelated_edit() {
        let text = FILE.replace("due: 2026-10-15", "due: 2026/10/15");
        let original = Tip::parse("tips/a.md", None, &text).unwrap();
        let edited = apply(
            &original,
            TipEdit {
                title: "Renamed".into(),
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(edited.front.due.as_deref(), Some("2026/10/15"));
    }

    #[test]
    fn empty_due_clears_it() {
        for due in [None, Some("   ".to_string())] {
            let edited = apply(&tip(), TipEdit { due, ..edit() }).unwrap();
            assert_eq!(edited.front.due, None);
            assert_eq!(edited.due_at, None);
        }
    }

    #[test]
    fn rejects_bad_input() {
        let bad = |e: TipEdit| apply(&tip(), e).unwrap_err();
        assert!(bad(TipEdit {
            title: "   ".into(),
            ..edit()
        })
        .contains("title"));
        assert!(bad(TipEdit {
            title: "x".repeat(MAX_TITLE_CHARS + 1),
            ..edit()
        })
        .contains("too long"));
        assert!(bad(TipEdit {
            due: Some("next tuesday".into()),
            ..edit()
        })
        .contains("not a date"));
        assert!(bad(TipEdit {
            location: Some("x".repeat(MAX_LOCATION_CHARS + 1)),
            ..edit()
        })
        .contains("location"));
        assert!(bad(TipEdit {
            tags: vec!["two words".into()],
            ..edit()
        })
        .contains("spaces"));
        assert!(bad(TipEdit {
            tags: (0..=MAX_TAGS).map(|i| format!("t{i}")).collect(),
            ..edit()
        })
        .contains("tags"));
        assert!(bad(TipEdit {
            body: "x".repeat(MAX_BODY_CHARS + 1),
            ..edit()
        })
        .contains("too long"));
    }

    #[test]
    fn deserialises_from_the_frontend_shape() {
        let e: TipEdit = serde_json::from_str(
            r#"{"title":"T","kind":"note","priority":"low","due":null,"tags":["a"],"body":"b"}"#,
        )
        .unwrap();
        assert_eq!(e.kind, Kind::Note);
        assert_eq!(e.location, None);
        assert!(serde_json::from_str::<TipEdit>(
            r#"{"title":"T","kind":"chore","priority":"low"}"#
        )
        .is_err());
    }
}
