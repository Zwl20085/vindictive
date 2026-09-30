//! Parse the one-line quick-capture syntax into a new tip.
//!
//! ```text
//! Check coil temperature after run 3 #lab !high @tomorrow 09:00 ^"Lab 302"
//! ```
//! - `#tag`            adds a tag (repeatable)
//! - `!high` / `!low`  sets priority
//! - `@today`, `@tomorrow`, `@2026-10-15`, optionally followed by `HH:MM`
//! - `^location`       sets location; quote it for spaces
//! - `>kind`           one of task, deadline, note, reading, event
//! - a bare arXiv id / URL or DOI sets `arxiv` / `doi` and kind reading

use std::sync::LazyLock;

use chrono::{Duration, NaiveDateTime, NaiveTime};
use regex::Regex;

use super::timeparse::{format_dt, parse_datetime};
use super::tip::{FrontMatter, Kind, Priority};

/// A captured date without a time means end of day, like a bare `due` date in
/// a tip file; `@today` typed in the afternoon must not be born overdue.
const DEFAULT_CAPTURE_TIME: (u32, u32) = (23, 59);

pub fn parse(input: &str, now: NaiveDateTime) -> Option<FrontMatter> {
    let tokens = tokenize(input);
    let mut front = FrontMatter {
        created: Some(format_dt(now)),
        ..Default::default()
    };
    let mut title_words: Vec<String> = Vec::new();
    let mut pending_date: Option<NaiveDateTime> = None;

    for tok in tokens {
        if let Some(tag) = tok.strip_prefix('#').filter(|t| !t.is_empty()) {
            front.tags.push(tag.to_string());
        } else if let Some(p) = tok.strip_prefix('!') {
            match p.to_ascii_lowercase().as_str() {
                "high" | "h" => front.priority = Priority::High,
                "low" | "l" => front.priority = Priority::Low,
                _ => title_words.push(tok),
            }
        } else if let Some(loc) = tok.strip_prefix('^').filter(|l| !l.is_empty()) {
            front.location = Some(loc.to_string());
        } else if let Some(k) = tok.strip_prefix('>') {
            match parse_kind(k) {
                Some(kind) => front.kind = kind,
                None => title_words.push(tok),
            }
        } else if let Some(d) = tok.strip_prefix('@') {
            match parse_date_word(d, now) {
                Some(dt) => pending_date = Some(dt),
                None => title_words.push(tok),
            }
        } else if pending_date.is_some() && looks_like_time(&tok) {
            let t = NaiveTime::parse_from_str(&tok, "%H:%M").ok()?;
            pending_date = pending_date.map(|d| d.date().and_time(t));
        } else if let Some(id) = arxiv_id(&tok) {
            front.arxiv = Some(id);
            front.kind = Kind::Reading;
        } else if let Some(doi) = doi(&tok) {
            front.doi = Some(doi);
            front.kind = Kind::Reading;
        } else {
            title_words.push(tok);
        }
    }

    if let Some(due) = pending_date {
        front.due = Some(format_dt(due));
        if front.kind == Kind::Task {
            front.kind = Kind::Deadline;
        }
    }
    front.title = title_words.join(" ").trim().to_string();
    if front.title.is_empty() {
        front.title = front
            .arxiv
            .as_ref()
            .map(|a| format!("arXiv {a}"))
            .or_else(|| front.doi.clone())
            .unwrap_or_default();
    }
    (!front.title.is_empty()).then_some(front)
}

fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in input.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_kind(k: &str) -> Option<Kind> {
    match k.to_ascii_lowercase().as_str() {
        "task" => Some(Kind::Task),
        "deadline" | "due" => Some(Kind::Deadline),
        "note" => Some(Kind::Note),
        "reading" | "read" | "paper" => Some(Kind::Reading),
        "event" | "meeting" => Some(Kind::Event),
        _ => None,
    }
}

fn parse_date_word(d: &str, now: NaiveDateTime) -> Option<NaiveDateTime> {
    let base_time = NaiveTime::from_hms_opt(DEFAULT_CAPTURE_TIME.0, DEFAULT_CAPTURE_TIME.1, 0)?;
    match d.to_ascii_lowercase().as_str() {
        "today" | "tod" => Some(now.date().and_time(base_time)),
        "tomorrow" | "tom" | "tmr" => Some((now + Duration::days(1)).date().and_time(base_time)),
        "nextweek" => Some((now + Duration::weeks(1)).date().and_time(base_time)),
        other => parse_datetime(other, DEFAULT_CAPTURE_TIME),
    }
}

fn looks_like_time(tok: &str) -> bool {
    tok.len() == 5 && tok.as_bytes()[2] == b':' && tok[..2].parse::<u8>().is_ok()
}

/// Extract an arXiv identifier from a bare id or an arxiv.org URL.
pub fn arxiv_id(tok: &str) -> Option<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(?:arxiv\.org/(?:abs|pdf)/|arxiv:)?(\d{4}\.\d{4,5})(v\d+)?(?:\.pdf)?$")
            .expect("valid arxiv regex")
    });
    let caps = RE.captures(tok)?;
    let is_bare = !tok.contains('/') && !tok.contains(':');
    let is_url = tok.to_ascii_lowercase().contains("arxiv");
    (is_bare || is_url).then(|| caps[1].to_string())
}

/// Extract a DOI from a bare DOI or a doi.org URL.
pub fn doi(tok: &str) -> Option<String> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(10\.\d{4,9}/[^\s\]\)>]+)").expect("valid doi regex"));
    let m = RE.captures(tok)?;
    let d = m[1].trim_end_matches(['.', ',', ';']).to_string();
    Some(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn full_syntax() {
        let now = dt("2026-10-01 15:00");
        let f = parse(
            "Check coil temp after run 3 #lab #thermal !high @tomorrow 09:30 ^\"Lab 302\"",
            now,
        )
        .unwrap();
        assert_eq!(f.title, "Check coil temp after run 3");
        assert_eq!(f.tags, vec!["lab", "thermal"]);
        assert_eq!(f.priority, Priority::High);
        assert_eq!(f.due.as_deref(), Some("2026-10-02 09:30"));
        assert_eq!(f.location.as_deref(), Some("Lab 302"));
        assert_eq!(
            f.kind,
            Kind::Deadline,
            "a task with a date becomes a deadline"
        );
        assert_eq!(f.created.as_deref(), Some("2026-10-01 15:00"));
    }

    #[test]
    fn explicit_kind_and_absolute_date() {
        let now = dt("2026-10-01 15:00");
        let f = parse(">event Group meeting @2026-10-05 10:00", now).unwrap();
        assert_eq!(f.kind, Kind::Event);
        assert_eq!(f.due.as_deref(), Some("2026-10-05 10:00"));
        let f = parse("Plain note >note", now).unwrap();
        assert_eq!(f.kind, Kind::Note);
        assert!(f.due.is_none());
    }

    #[test]
    fn arxiv_and_doi_make_reading_tips() {
        let now = dt("2026-10-01 15:00");
        let f = parse("https://arxiv.org/abs/2401.12345v2", now).unwrap();
        assert_eq!(f.arxiv.as_deref(), Some("2401.12345"));
        assert_eq!(f.kind, Kind::Reading);
        assert_eq!(f.title, "arXiv 2401.12345");
        let f = parse("Read this 10.1109/TIE.2024.1234567", now).unwrap();
        assert_eq!(f.doi.as_deref(), Some("10.1109/TIE.2024.1234567"));
        assert_eq!(f.title, "Read this");
        let f = parse("https://doi.org/10.1000/xyz123.", now).unwrap();
        assert_eq!(f.doi.as_deref(), Some("10.1000/xyz123"));
    }

    #[test]
    fn unknown_markers_stay_in_title() {
        let now = dt("2026-10-01 15:00");
        let f = parse("Ask about !budget @noon >thing", now).unwrap();
        assert_eq!(f.title, "Ask about !budget @noon >thing");
    }

    #[test]
    fn empty_is_none() {
        let now = dt("2026-10-01 15:00");
        assert!(parse("   ", now).is_none());
        assert!(parse("#tag !high", now).is_none());
    }

    #[test]
    fn arxiv_id_forms() {
        assert_eq!(arxiv_id("2401.12345"), Some("2401.12345".into()));
        assert_eq!(arxiv_id("arXiv:2401.12345"), Some("2401.12345".into()));
        assert_eq!(
            arxiv_id("https://arxiv.org/pdf/2401.12345.pdf"),
            Some("2401.12345".into())
        );
        assert_eq!(arxiv_id("https://example.org/2401.12345"), None);
        assert_eq!(arxiv_id("1234.5"), None);
    }

    #[test]
    fn bare_capture_date_means_end_of_day() {
        let afternoon = dt("2026-10-02 15:00");
        let f = parse("Call supplier @today", afternoon).unwrap();
        assert_eq!(f.due.as_deref(), Some("2026-10-02 23:59"));
        let f = parse("Submit @2026-10-15", afternoon).unwrap();
        assert_eq!(f.due.as_deref(), Some("2026-10-15 23:59"));
        let f = parse("Standup @tomorrow 09:30", afternoon).unwrap();
        assert_eq!(f.due.as_deref(), Some("2026-10-03 09:30"));
    }
}
