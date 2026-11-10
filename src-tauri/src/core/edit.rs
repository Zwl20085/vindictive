//! In-app editing of a tip: validate the fields the edit form sends and
//! build the new tip. Every frontmatter key the form does not show (order,
//! paper ids, snooze, done, created and unknown keys) is carried over
//! untouched, and a shown field the user left as it was keeps the exact text
//! of the file, so a save that changes one field rewrites only that field.

use serde::Deserialize;

use super::prompt::title_from_prompt;
use super::recur;
use super::timeparse::{is_offset, parse_datetime, parse_due, parse_offset};
use super::tip::{FrontMatter, Kind, Priority, Tip};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_LOCATION_CHARS: usize = 200;
pub const MAX_TAGS: usize = 32;
pub const MAX_TAG_CHARS: usize = 64;
pub const MAX_BODY_CHARS: usize = 100_000;
pub const MAX_DUE_CHARS: usize = 40;
pub const MAX_REMINDS: usize = 32;
pub const MAX_REPEAT_CHARS: usize = 100;
pub const MAX_LINKS: usize = 32;
pub const MAX_LINK_CHARS: usize = 500;
pub const MAX_IMAGES: usize = 32;
pub const MAX_IMAGE_CHARS: usize = 500;
pub const MAX_HIGHLIGHTS: usize = 32;
pub const MAX_HIGHLIGHT_CHARS: usize = 64;
pub const MIN_HIGHLIGHT_CHARS: usize = 2;
/// Tile size overrides, as written in `size:`.
pub const TILE_SIZES: [&str; 3] = ["sm", "md", "wide"];

/// The fields of the edit form, as the frontend sends them. Every field but
/// the first three may be missing (an older form), which means empty.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
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
    /// Offsets from `due` (`-1d`, `-2h`) or dates; blank entries dropped.
    #[serde(default)]
    pub remind: Vec<String>,
    /// A `recur` rule (`weekly on mon`); `None` or blank clears it.
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub links: Vec<String>,
    /// Relative to the tips folder, or an `https://` URL.
    #[serde(default)]
    pub images: Vec<String>,
    /// `#rgb` / `#rrggbb`; `None` or blank clears it.
    #[serde(default)]
    pub color: Option<String>,
    /// `sm`, `md` or `wide`; `None` or blank clears it.
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub highlight: Vec<String>,
    #[serde(default)]
    pub body: String,
}

/// Validate `edit` and return the edited copy of `tip`. The file path, and
/// so the id, never changes: renaming the title keeps the file name.
pub fn apply(tip: &Tip, edit: TipEdit) -> Result<Tip, String> {
    let body = edit.body.replace("\r\n", "\n");
    check_len("text", &body, MAX_BODY_CHARS)?;
    let title = title_for(edit.title.trim(), edit.kind, tip.is_question(), &body)?;
    check_len("title", &title, MAX_TITLE_CHARS)?;
    let location = optional(edit.location.as_deref());
    if let Some(l) = &location {
        check_len("location", l, MAX_LOCATION_CHARS)?;
    }
    let old = &tip.front;
    let due = due(tip, edit.due.as_deref())?;
    let has_due = due.as_deref().and_then(parse_due).is_some();
    let front = FrontMatter {
        title,
        kind: edit.kind,
        priority: edit.priority,
        location,
        tags: tags(&edit.tags)?,
        remind: keep_list(&old.remind, &edit.remind, |l| remind(l, has_due))?,
        repeat: keep_opt(&old.repeat, edit.repeat.as_deref(), repeat)?,
        links: keep_list(&old.links, &edit.links, links)?,
        images: keep_list(&old.images, &edit.images, images)?,
        color: keep_opt(&old.color, edit.color.as_deref(), color)?,
        size: keep_opt(&old.size, edit.size.as_deref(), |s| tile_size(&s))?,
        highlight: keep_list(&old.highlight, &edit.highlight, highlight_words)?,
        due,
        ..old.clone()
    };
    Ok(Tip::from_parts(&tip.path, tip.sha.clone(), front, body))
}

/// The title as typed; for an agent tip left without one, derived from the
/// prompt (`core::prompt`), so the prompt alone is enough. A question tip is
/// never an agent task, so it always needs a title, whatever its kind.
fn title_for(typed: &str, kind: Kind, question: bool, body: &str) -> Result<String, String> {
    match (typed.is_empty(), kind) {
        (false, _) => Ok(typed.to_string()),
        (true, Kind::Agent) if !question => title_from_prompt(body)
            .ok_or_else(|| "an agent tip needs a prompt (or a title)".to_string()),
        (true, _) => Err("a tip needs a title".into()),
    }
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

/// Entries trimmed, blank ones dropped.
fn cleaned(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// A list field: the file's own strings when the form shows the same values
/// (even ones that would not validate today, such as a reminder the file
/// already warns about), else the validated new list.
fn keep_list(
    old: &[String],
    new: &[String],
    check: impl FnOnce(Vec<String>) -> Result<Vec<String>, String>,
) -> Result<Vec<String>, String> {
    let new = cleaned(new);
    if new == cleaned(old) {
        return Ok(old.to_vec());
    }
    check(new)
}

/// A single optional field, with the same "unchanged keeps the file's text"
/// rule as `keep_list`. Blank clears it.
fn keep_opt(
    old: &Option<String>,
    new: Option<&str>,
    check: impl FnOnce(String) -> Result<String, String>,
) -> Result<Option<String>, String> {
    let new = optional(new);
    if new == optional(old.as_deref()) {
        return Ok(old.clone());
    }
    new.map(check).transpose()
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

/// Each reminder is an offset from the due date or an absolute date.
fn remind(list: Vec<String>, has_due: bool) -> Result<Vec<String>, String> {
    if list.len() > MAX_REMINDS {
        return Err(format!("at most {MAX_REMINDS} reminders"));
    }
    for entry in &list {
        check_len("reminder", entry, MAX_DUE_CHARS)?;
        if is_offset(entry) && parse_offset(entry).is_some() {
            if !has_due {
                return Err(format!(
                    "remind {entry:?} is relative to the due date; set a due date first"
                ));
            }
        } else if parse_datetime(entry, (9, 0)).is_none() {
            return Err(format!(
                "remind {entry:?} is not an offset like -1d or a date"
            ));
        }
    }
    Ok(list)
}

fn repeat(rule: String) -> Result<String, String> {
    check_len("repeat rule", &rule, MAX_REPEAT_CHARS)?;
    if recur::parse(&rule).is_none() {
        return Err(format!(
            "repeat {rule:?} is not a rule like daily, weekdays, weekly on mon,thu, monthly on 1 or yearly on 03-15"
        ));
    }
    Ok(rule)
}

fn links(list: Vec<String>) -> Result<Vec<String>, String> {
    if list.len() > MAX_LINKS {
        return Err(format!("at most {MAX_LINKS} links"));
    }
    for link in &list {
        check_len("link", link, MAX_LINK_CHARS)?;
    }
    Ok(list)
}

fn images(list: Vec<String>) -> Result<Vec<String>, String> {
    if list.len() > MAX_IMAGES {
        return Err(format!("at most {MAX_IMAGES} images"));
    }
    for image in &list {
        check_len("image path", image, MAX_IMAGE_CHARS)?;
        if !is_image_ref(image) {
            return Err(format!(
                "image {image:?} must be a path inside the tips folder (figures/plot.png) or an https:// URL"
            ));
        }
    }
    Ok(list)
}

/// An `https://` URL, or a relative path that stays inside the tips folder:
/// no leading slash, no drive letter or scheme (`:`), no `..` component.
pub fn is_image_ref(value: &str) -> bool {
    if let Some(rest) = value.strip_prefix("https://") {
        return !rest.trim().is_empty();
    }
    !value.starts_with(['/', '\\'])
        && !value.contains(':')
        && !value.split(['/', '\\']).any(|part| part.trim() == "..")
}

fn color(value: String) -> Result<String, String> {
    if is_hex_color(&value) {
        Ok(value)
    } else {
        Err(format!("{value:?} is not a hex colour like #4A3B6B"))
    }
}

/// `#rgb` or `#rrggbb`.
pub fn is_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// A tile size override, trimmed and lowercased; an error unless it is one
/// of `TILE_SIZES`.
pub fn tile_size(value: &str) -> Result<String, String> {
    let value = value.trim().to_ascii_lowercase();
    if TILE_SIZES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(format!("size must be one of {}", TILE_SIZES.join(", ")))
    }
}

/// Words or phrases to highlight (a tip's `highlight:` or the global list in
/// Settings): trimmed, blank entries and case-insensitive duplicates dropped,
/// each 2 to 64 characters, at most 32.
pub fn highlight_words(raw: Vec<String>) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for word in cleaned(&raw) {
        let folded = word.to_lowercase();
        if out.iter().any(|w| w.to_lowercase() == folded) {
            continue;
        }
        if word.chars().count() < MIN_HIGHLIGHT_CHARS {
            return Err(format!(
                "highlight {word:?} is too short (at least {MIN_HIGHLIGHT_CHARS} characters)"
            ));
        }
        check_len("highlight", &word, MAX_HIGHLIGHT_CHARS)?;
        out.push(word);
    }
    if out.len() > MAX_HIGHLIGHTS {
        return Err(format!("at most {MAX_HIGHLIGHTS} highlight words"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    const FILE: &str = "---\ntitle: Submit camera-ready\nkind: deadline\npriority: high\ndue: 2026-10-15\nremind:\n  - -1d\nlocation: Lab 302\nlinks:\n  - https://example.org\nimages: [fig.png]\ntags: [ecce, paper]\nrepeat: weekly on mon\ncolor: '#4A3B6B'\nsize: wide\norder: 2.5\narxiv: '2401.00001'\nsnoozed_until: 2026-10-01 12:00\ndone_at: 2026-09-30 10:00\ncreated: 2026-09-01 09:00\ncustom_key: keep me\n---\n\nOld body.\n";

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
            remind: vec!["-1d".into()],
            repeat: Some("weekly on mon".into()),
            links: vec!["https://example.org".into()],
            images: vec!["fig.png".into()],
            color: Some("#4A3B6B".into()),
            size: Some("wide".into()),
            highlight: Vec::new(),
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
                ..edit()
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
    fn edits_the_extra_fields() {
        let edited = apply(
            &tip(),
            TipEdit {
                remind: vec![" -2h ".into(), "".into(), "2026-10-14 09:00".into()],
                repeat: Some(" monthly on 1 ".into()),
                links: vec![" https://a.org ".into(), "  ".into(), "doi:10.1/x".into()],
                images: vec!["figures/plot.svg".into(), "https://x.org/a.png".into()],
                color: Some(" #abc ".into()),
                size: Some(" SM ".into()),
                highlight: vec![
                    " Reviewer 2 ".into(),
                    "reviewer 2".into(),
                    "".into(),
                    "rebuttal".into(),
                ],
                ..edit()
            },
        )
        .unwrap();
        let f = &edited.front;
        assert_eq!(f.remind, vec!["-2h", "2026-10-14 09:00"]);
        assert_eq!(
            edited.remind_at,
            vec![dt("2026-10-14 09:00"), dt("2026-10-15 21:59")]
        );
        assert_eq!(f.repeat.as_deref(), Some("monthly on 1"));
        assert_eq!(f.links, vec!["https://a.org", "doi:10.1/x"]);
        assert_eq!(f.images, vec!["figures/plot.svg", "https://x.org/a.png"]);
        assert_eq!(f.color.as_deref(), Some("#abc"));
        assert_eq!(f.size.as_deref(), Some("sm"));
        assert_eq!(f.highlight, vec!["Reviewer 2", "rebuttal"]);
        assert!(edited.warnings.is_empty(), "{:?}", edited.warnings);
        let again = Tip::parse(&edited.path, None, &edited.to_markdown().unwrap()).unwrap();
        assert_eq!(again.front, edited.front);
    }

    #[test]
    fn blank_extra_fields_clear_them() {
        let edited = apply(
            &tip(),
            TipEdit {
                remind: vec![" ".into()],
                repeat: Some("  ".into()),
                links: Vec::new(),
                images: Vec::new(),
                color: None,
                size: Some(String::new()),
                ..edit()
            },
        )
        .unwrap();
        let f = &edited.front;
        assert!(f.remind.is_empty() && edited.remind_at.is_empty());
        assert_eq!(
            (f.repeat.as_ref(), f.color.as_ref(), f.size.as_ref()),
            (None, None, None)
        );
        assert!(f.links.is_empty() && f.images.is_empty() && f.highlight.is_empty());
        let text = edited.to_markdown().unwrap();
        for key in ["remind:", "repeat:", "links:", "images:", "color:", "size:"] {
            assert!(!text.contains(key), "{key} in {text}");
        }
    }

    #[test]
    fn unchanged_extra_fields_keep_the_files_text() {
        // Entries that would not validate as new input (an unparsable
        // reminder, a bad rule, an absolute image path) survive an edit that
        // did not touch them; the tip keeps its warnings instead of failing.
        let text = FILE
            .replace("  - -1d", "  - someday")
            .replace("repeat: weekly on mon", "repeat: fortnightly")
            .replace("images: [fig.png]", "images: [/abs/fig.png]")
            .replace("size: wide", "size: Wide");
        let original = Tip::parse("tips/a.md", None, &text).unwrap();
        assert!(!original.warnings.is_empty());
        let edited = apply(
            &original,
            TipEdit {
                title: "Renamed".into(),
                remind: vec![" someday ".into(), "".into()],
                repeat: Some("fortnightly ".into()),
                images: vec!["/abs/fig.png".into()],
                size: Some("Wide".into()),
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(edited.front.remind, original.front.remind);
        assert_eq!(edited.front.repeat, original.front.repeat);
        assert_eq!(edited.front.images, original.front.images);
        assert_eq!(edited.front.size.as_deref(), Some("Wide"));
        // But touching one of them validates the new value.
        let err = apply(
            &original,
            TipEdit {
                remind: vec!["someday".into(), "-1h".into()],
                ..edit()
            },
        )
        .unwrap_err();
        assert!(err.contains("someday"), "{err}");
    }

    #[test]
    fn rejects_bad_extra_fields() {
        let bad = |e: TipEdit| apply(&tip(), e).unwrap_err();
        assert_eq!(
            bad(TipEdit {
                remind: vec!["soon".into()],
                ..edit()
            }),
            r#"remind "soon" is not an offset like -1d or a date"#
        );
        assert!(bad(TipEdit {
            due: None,
            remind: vec!["-1d".into(), "-2h".into()],
            ..edit()
        })
        .contains("due date"));
        // An absolute reminder needs no due date.
        let ok = apply(
            &tip(),
            TipEdit {
                due: None,
                remind: vec!["2026-10-14".into()],
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(ok.remind_at, vec![dt("2026-10-14 09:00")]);
        assert!(bad(TipEdit {
            repeat: Some("every blue moon".into()),
            ..edit()
        })
        .contains("repeat"));
        assert!(bad(TipEdit {
            links: (0..=MAX_LINKS).map(|i| format!("https://x/{i}")).collect(),
            ..edit()
        })
        .contains("links"));
        assert!(bad(TipEdit {
            links: vec!["x".repeat(MAX_LINK_CHARS + 1)],
            ..edit()
        })
        .contains("too long"));
        for image in [
            "../secret.png",
            "figures/../../x.png",
            "/etc/x.png",
            "\\server\\x.png",
            "C:/x.png",
            "http://x.org/a.png",
            "file:///x.png",
            "https://",
        ] {
            assert!(
                bad(TipEdit {
                    images: vec![image.into()],
                    ..edit()
                })
                .contains("image"),
                "{image}"
            );
        }
        assert!(bad(TipEdit {
            images: (0..=MAX_IMAGES).map(|i| format!("f{i}.png")).collect(),
            ..edit()
        })
        .contains("images"));
        for color in ["red", "#12345", "4A3B6B"] {
            assert!(bad(TipEdit {
                color: Some(color.into()),
                ..edit()
            })
            .contains("hex"));
        }
        assert!(bad(TipEdit {
            size: Some("huge".into()),
            ..edit()
        })
        .contains("size"));
        assert!(bad(TipEdit {
            highlight: vec!["x".into()],
            ..edit()
        })
        .contains("too short"));
        assert!(bad(TipEdit {
            highlight: vec!["x".repeat(MAX_HIGHLIGHT_CHARS + 1)],
            ..edit()
        })
        .contains("too long"));
        assert!(bad(TipEdit {
            highlight: (0..=MAX_HIGHLIGHTS).map(|i| format!("w{i}")).collect(),
            ..edit()
        })
        .contains("highlight"));
    }

    #[test]
    fn highlight_word_rules() {
        assert_eq!(
            highlight_words(vec![" ab ".into(), "AB".into(), "Ab c".into()]).unwrap(),
            vec!["ab", "Ab c"]
        );
        // Duplicates do not count against the limit.
        let many: Vec<String> = (0..MAX_HIGHLIGHTS)
            .flat_map(|i| [format!("w{i}"), format!("W{i}")])
            .collect();
        assert_eq!(highlight_words(many).unwrap().len(), MAX_HIGHLIGHTS);
        assert!(highlight_words(vec!["审".into()]).is_err());
        assert_eq!(highlight_words(vec!["审稿".into()]).unwrap(), vec!["审稿"]);
        assert!(highlight_words(Vec::new()).unwrap().is_empty());
    }

    #[test]
    fn shared_validators() {
        assert!(is_hex_color("#4A3B6B"));
        assert!(is_hex_color("#abc"));
        assert!(!is_hex_color("4A3B6B"));
        assert!(!is_hex_color("#12345"));
        assert!(!is_hex_color("#GGGGGG"));
        assert_eq!(tile_size(" Wide ").unwrap(), "wide");
        assert!(tile_size("xl").is_err());
        assert!(is_image_ref("figures/a b.png"));
        assert!(is_image_ref("a..b.png"));
        assert!(!is_image_ref("figures\\..\\x.png"));
    }

    #[test]
    fn deserialises_from_the_frontend_shape() {
        let e: TipEdit = serde_json::from_str(
            r#"{"title":"T","kind":"note","priority":"low","due":null,"tags":["a"],"body":"b"}"#,
        )
        .unwrap();
        assert_eq!(e.kind, Kind::Note);
        assert_eq!(e.location, None);
        // The pre-1.0 form had no extra fields: they read as empty.
        assert_eq!(
            e,
            TipEdit {
                title: "T".into(),
                kind: Kind::Note,
                priority: Priority::Low,
                tags: vec!["a".into()],
                body: "b".into(),
                ..Default::default()
            }
        );
        let full: TipEdit = serde_json::from_str(
            r##"{"title":"T","kind":"task","priority":"high","due":"2026-10-15","location":null,
            "tags":[],"remind":["-1d"],"repeat":null,"links":["https://a.org"],
            "images":["f.png"],"color":"#abc","size":"md","highlight":["ab"],"body":""}"##,
        )
        .unwrap();
        assert_eq!(full.remind, vec!["-1d"]);
        assert_eq!(full.repeat, None);
        assert_eq!(full.links, vec!["https://a.org"]);
        assert_eq!(full.images, vec!["f.png"]);
        assert_eq!(full.color.as_deref(), Some("#abc"));
        assert_eq!(full.size.as_deref(), Some("md"));
        assert_eq!(full.highlight, vec!["ab"]);
        assert!(serde_json::from_str::<TipEdit>(
            r#"{"title":"T","kind":"chore","priority":"low"}"#
        )
        .is_err());
    }

    #[test]
    fn agent_tips_take_their_title_from_the_prompt_when_left_blank() {
        let prompt = "## Survey **hairpin** AC loss

Find three recent papers.
";
        let edited = apply(
            &tip(),
            TipEdit {
                title: "  ".into(),
                kind: Kind::Agent,
                body: prompt.into(),
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(edited.front.title, "Survey hairpin AC loss");
        // The body is the prompt as typed (line ends unified, nothing added).
        assert_eq!(
            edited.body,
            "## Survey **hairpin** AC loss

Find three recent papers.
"
        );
        // A title the user typed wins.
        let named = apply(
            &tip(),
            TipEdit {
                title: "My survey".into(),
                kind: Kind::Agent,
                body: prompt.into(),
                ..edit()
            },
        )
        .unwrap();
        assert_eq!(named.front.title, "My survey");
        // Neither title nor prompt: refused, naming the prompt.
        let err = apply(
            &tip(),
            TipEdit {
                title: "".into(),
                kind: Kind::Agent,
                body: " 
"
                .into(),
                ..edit()
            },
        )
        .unwrap_err();
        assert!(err.contains("prompt"), "{err}");
        // Other kinds still need a title.
        let err = apply(
            &tip(),
            TipEdit {
                title: "".into(),
                kind: Kind::Note,
                body: prompt.into(),
                ..edit()
            },
        )
        .unwrap_err();
        assert!(err.contains("title"), "{err}");
    }

    #[test]
    fn a_question_tip_with_kind_agent_still_needs_a_title() {
        let question = Tip::parse(
            "questions/q.md",
            Some("1".into()),
            "---
title: Q
kind: agent
---
Answer:
",
        )
        .unwrap();
        let err = apply(
            &question,
            TipEdit {
                title: " ".into(),
                kind: Kind::Agent,
                body: "Answer: fine
"
                .into(),
                ..edit()
            },
        )
        .unwrap_err();
        assert!(err.contains("title"), "{err}");
    }
}
