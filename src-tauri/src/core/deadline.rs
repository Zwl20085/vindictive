//! Urgency classification for due dates. The UI maps each level to a colour.

use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    /// No due date.
    None,
    /// More than a week away.
    Later,
    /// Within seven days.
    Soon,
    /// Within 48 hours.
    Critical,
    /// Due time has passed.
    Overdue,
}

pub const CRITICAL_WINDOW: Duration = Duration::hours(48);
pub const SOON_WINDOW: Duration = Duration::days(7);

pub fn urgency(due: Option<NaiveDateTime>, now: NaiveDateTime) -> Urgency {
    let Some(due) = due else { return Urgency::None };
    let left = due - now;
    if left <= Duration::zero() {
        Urgency::Overdue
    } else if left <= CRITICAL_WINDOW {
        Urgency::Critical
    } else if left <= SOON_WINDOW {
        Urgency::Soon
    } else {
        Urgency::Later
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn classifies() {
        let now = dt("2026-10-01 12:00");
        assert_eq!(urgency(None, now), Urgency::None);
        assert_eq!(urgency(Some(dt("2026-10-20 12:00")), now), Urgency::Later);
        assert_eq!(urgency(Some(dt("2026-10-08 12:00")), now), Urgency::Soon);
        assert_eq!(urgency(Some(dt("2026-10-08 12:01")), now), Urgency::Later);
        assert_eq!(
            urgency(Some(dt("2026-10-03 12:00")), now),
            Urgency::Critical
        );
        assert_eq!(urgency(Some(dt("2026-10-01 12:00")), now), Urgency::Overdue);
        assert_eq!(urgency(Some(dt("2026-09-30 12:00")), now), Urgency::Overdue);
        assert!(Urgency::Overdue > Urgency::Critical && Urgency::Critical > Urgency::Soon);
    }
}
