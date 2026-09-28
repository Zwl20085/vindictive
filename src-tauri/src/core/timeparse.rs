//! Lenient parsing of the date strings people actually type into frontmatter.
//!
//! All times are local naive times: what the wall clock in the lab shows.

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};

const DATETIME_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%dT%H:%M:%S",
    "%Y/%m/%d %H:%M",
];
const DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%Y/%m/%d"];

const END_OF_DAY: (u32, u32) = (23, 59);
const MORNING: (u32, u32) = (9, 0);

/// Parse a `due` value. A date without a time means end of that day.
pub fn parse_due(s: &str) -> Option<NaiveDateTime> {
    parse_datetime(s, END_OF_DAY)
}

/// Parse a `remind` value. Accepts absolute datetimes, dates (09:00 assumed),
/// or offsets relative to `due` such as `-1d`, `-2h`, `-30m`, `-1w`.
pub fn parse_remind(s: &str, due: Option<NaiveDateTime>) -> Option<NaiveDateTime> {
    let s = s.trim();
    if let Some(offset) = parse_offset(s) {
        return due.map(|d| d + offset);
    }
    parse_datetime(s, MORNING)
}

/// Parse an absolute datetime; a bare date takes `default_time`.
pub fn parse_datetime(s: &str, default_time: (u32, u32)) -> Option<NaiveDateTime> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_local());
    }
    for f in DATETIME_FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            return Some(dt);
        }
    }
    for f in DATE_FORMATS {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            let t = NaiveTime::from_hms_opt(default_time.0, default_time.1, 0)?;
            return Some(d.and_time(t));
        }
    }
    None
}

/// `-1d`, `-2h`, `-30m`, `-1w`, `+1h` → signed duration.
pub fn parse_offset(s: &str) -> Option<Duration> {
    let s = s.trim();
    let (sign, rest) = match s.chars().next()? {
        '-' => (-1, &s[1..]),
        '+' => (1, &s[1..]),
        _ => return None,
    };
    let unit = rest.chars().last()?;
    let n: i64 = rest[..rest.len() - unit.len_utf8()].trim().parse().ok()?;
    let d = match unit {
        'm' => Duration::minutes(n),
        'h' => Duration::hours(n),
        'd' => Duration::days(n),
        'w' => Duration::weeks(n),
        _ => return None,
    };
    Some(d * sign)
}

/// Canonical form written back into frontmatter.
pub fn format_dt(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn due_formats() {
        assert_eq!(parse_due("2026-10-15"), Some(dt("2026-10-15 23:59")));
        assert_eq!(parse_due("2026-10-15 08:30"), Some(dt("2026-10-15 08:30")));
        assert_eq!(parse_due("2026-10-15T08:30"), Some(dt("2026-10-15 08:30")));
        assert_eq!(parse_due("2026/10/15"), Some(dt("2026-10-15 23:59")));
        assert_eq!(
            parse_due("2026-10-15T08:30:00+02:00"),
            Some(dt("2026-10-15 08:30"))
        );
        assert_eq!(parse_due("soon"), None);
    }

    #[test]
    fn remind_relative_to_due() {
        let due = Some(dt("2026-10-15 23:59"));
        assert_eq!(parse_remind("-1d", due), Some(dt("2026-10-14 23:59")));
        assert_eq!(parse_remind("-2h", due), Some(dt("2026-10-15 21:59")));
        assert_eq!(parse_remind("-30m", due), Some(dt("2026-10-15 23:29")));
        assert_eq!(parse_remind("-1w", due), Some(dt("2026-10-08 23:59")));
        assert_eq!(parse_remind("-1d", None), None);
        assert_eq!(
            parse_remind("2026-10-01", None),
            Some(dt("2026-10-01 09:00"))
        );
    }

    #[test]
    fn offsets() {
        assert_eq!(parse_offset("+1h"), Some(Duration::hours(1)));
        assert_eq!(parse_offset("-x"), None);
        assert_eq!(parse_offset("1d"), None);
        assert_eq!(parse_offset("-3y"), None);
    }

    #[test]
    fn format_roundtrip() {
        let t = dt("2026-01-02 03:04");
        assert_eq!(format_dt(t), "2026-01-02 03:04");
        assert_eq!(parse_due(&format_dt(t)), Some(t));
    }
}
