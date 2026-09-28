//! A tiny human-readable recurrence grammar.
//!
//! ```text
//! daily [at HH:MM]
//! weekdays [at HH:MM]
//! weekly on mon[,thu] [at HH:MM]
//! every monday [at HH:MM]            (alias of weekly on mon)
//! monthly on 15 [at HH:MM]
//! yearly on 03-15 [at HH:MM]
//! ```
//! Default time is 09:00. Day names accept 3-letter or full English names.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Daily { at: NaiveTime },
    Weekdays { at: NaiveTime },
    Weekly { days: Vec<Weekday>, at: NaiveTime },
    Monthly { day: u32, at: NaiveTime },
    Yearly { month: u32, day: u32, at: NaiveTime },
}

const DEFAULT_TIME: (u32, u32) = (9, 0);
const MAX_SEARCH_DAYS: i64 = 366 * 4;

pub fn parse(rule: &str) -> Option<Rule> {
    let text = rule.trim().to_ascii_lowercase();
    let (spec, at) = match text.split_once(" at ") {
        Some((spec, t)) => (spec.trim().to_string(), parse_time(t.trim())?),
        None => (
            text.clone(),
            NaiveTime::from_hms_opt(DEFAULT_TIME.0, DEFAULT_TIME.1, 0)?,
        ),
    };
    let words: Vec<&str> = spec.split_whitespace().collect();
    match words.as_slice() {
        ["daily"] | ["every", "day"] => Some(Rule::Daily { at }),
        ["weekdays"] | ["every", "weekday"] => Some(Rule::Weekdays { at }),
        ["weekly", "on", days] | ["every", days] | ["every", "week", "on", days] => {
            let days = parse_days(days)?;
            Some(Rule::Weekly { days, at })
        }
        ["monthly", "on", day] | ["every", "month", "on", day] => {
            let day: u32 = day
                .trim_end_matches(['s', 't', 'h', 'n', 'd', 'r'])
                .parse()
                .ok()?;
            (1..=31).contains(&day).then_some(Rule::Monthly { day, at })
        }
        ["yearly", "on", md] | ["every", "year", "on", md] => {
            let (m, d) = md.split_once('-')?;
            let (month, day) = (m.parse().ok()?, d.parse().ok()?);
            NaiveDate::from_ymd_opt(2024, month, day).map(|_| Rule::Yearly { month, day, at })
        }
        _ => None,
    }
}

fn parse_time(t: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(t, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M:%S"))
        .ok()
}

fn parse_days(s: &str) -> Option<Vec<Weekday>> {
    let mut days: Vec<Weekday> = s
        .split(',')
        .map(|d| parse_day(d.trim()))
        .collect::<Option<Vec<_>>>()?;
    days.sort_by_key(|d| d.num_days_from_monday());
    days.dedup();
    (!days.is_empty()).then_some(days)
}

fn parse_day(d: &str) -> Option<Weekday> {
    let d = d.trim_end_matches('s');
    match d.get(..3)? {
        "mon" => Some(Weekday::Mon),
        "tue" => Some(Weekday::Tue),
        "wed" => Some(Weekday::Wed),
        "thu" => Some(Weekday::Thu),
        "fri" => Some(Weekday::Fri),
        "sat" => Some(Weekday::Sat),
        "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

/// The first occurrence strictly after `after`.
pub fn next_occurrence(rule: &str, after: NaiveDateTime) -> Option<NaiveDateTime> {
    let rule = parse(rule)?;
    let mut date = after.date();
    for _ in 0..MAX_SEARCH_DAYS {
        if let Some(candidate) = occurrence_on(&rule, date) {
            if candidate > after {
                return Some(candidate);
            }
        }
        date += Duration::days(1);
    }
    None
}

fn occurrence_on(rule: &Rule, date: NaiveDate) -> Option<NaiveDateTime> {
    let (matches, at) = match rule {
        Rule::Daily { at } => (true, *at),
        Rule::Weekdays { at } => (date.weekday().number_from_monday() <= 5, *at),
        Rule::Weekly { days, at } => (days.contains(&date.weekday()), *at),
        Rule::Monthly { day, at } => (date.day() == *day, *at),
        Rule::Yearly { month, day, at } => (date.month() == *month && date.day() == *day, *at),
    };
    matches.then(|| date.and_time(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn parses_grammar() {
        assert_eq!(parse("daily"), Some(Rule::Daily { at: t(9, 0) }));
        assert_eq!(parse("Daily at 18:30"), Some(Rule::Daily { at: t(18, 30) }));
        assert_eq!(
            parse("weekdays at 08:00"),
            Some(Rule::Weekdays { at: t(8, 0) })
        );
        assert_eq!(
            parse("weekly on thu,mon at 10:00"),
            Some(Rule::Weekly {
                days: vec![Weekday::Mon, Weekday::Thu],
                at: t(10, 0)
            })
        );
        assert_eq!(
            parse("every monday"),
            Some(Rule::Weekly {
                days: vec![Weekday::Mon],
                at: t(9, 0)
            })
        );
        assert_eq!(
            parse("monthly on 1st"),
            Some(Rule::Monthly {
                day: 1,
                at: t(9, 0)
            })
        );
        assert_eq!(
            parse("yearly on 03-15"),
            Some(Rule::Yearly {
                month: 3,
                day: 15,
                at: t(9, 0)
            })
        );
        assert_eq!(parse("whenever"), None);
        assert_eq!(parse("monthly on 40"), None);
        assert_eq!(parse("yearly on 13-01"), None);
        assert_eq!(parse("daily at 25:00"), None);
    }

    #[test]
    fn next_daily_and_weekdays() {
        // 2026-10-02 is a Friday.
        assert_eq!(
            next_occurrence("daily", dt("2026-10-02 08:00")),
            Some(dt("2026-10-02 09:00"))
        );
        assert_eq!(
            next_occurrence("daily", dt("2026-10-02 09:00")),
            Some(dt("2026-10-03 09:00"))
        );
        assert_eq!(
            next_occurrence("weekdays at 08:00", dt("2026-10-02 10:00")),
            Some(dt("2026-10-05 08:00"))
        );
    }

    #[test]
    fn next_weekly_monthly_yearly() {
        assert_eq!(
            next_occurrence("weekly on mon,thu at 10:00", dt("2026-10-05 10:00")),
            Some(dt("2026-10-08 10:00"))
        );
        assert_eq!(
            next_occurrence("monthly on 31", dt("2026-02-10 00:00")),
            Some(dt("2026-03-31 09:00")),
            "months without a 31st are skipped"
        );
        assert_eq!(
            next_occurrence("yearly on 02-29 at 12:00", dt("2026-01-01 00:00")),
            Some(dt("2028-02-29 12:00"))
        );
    }

    #[test]
    fn bad_rule_has_no_occurrence() {
        assert_eq!(next_occurrence("sometimes", dt("2026-01-01 00:00")), None);
    }
}
