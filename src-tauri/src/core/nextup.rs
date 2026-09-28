//! Choose the single tip the researcher should do next.
//!
//! The rule is deliberately simple and explainable: urgency dominates,
//! priority breaks ties, then kind, then the earlier due date, then title.

use chrono::NaiveDateTime;

use super::deadline::{urgency, Urgency};
use super::tip::{Kind, Priority, Tip};

/// Numeric score; higher means do it sooner.
pub fn score(tip: &Tip, now: NaiveDateTime) -> i64 {
    let urgency_points = match urgency(tip.due_at, now) {
        Urgency::Overdue => 1000,
        Urgency::Critical => 500,
        Urgency::Soon => 200,
        Urgency::Later => 50,
        Urgency::None => 10,
    };
    let priority_points = match tip.front.priority {
        Priority::High => 30,
        Priority::Normal => 10,
        Priority::Low => 0,
    };
    let kind_points = match tip.front.kind {
        Kind::Deadline => 5,
        Kind::Event => 3,
        Kind::Task => 2,
        Kind::Reading => 1,
        Kind::Note => 0,
    };
    urgency_points + priority_points + kind_points
}

/// Tips eligible for the board: open and not snoozed.
pub fn visible(tips: &[Tip], now: NaiveDateTime) -> Vec<&Tip> {
    tips.iter()
        .filter(|t| t.is_open() && !t.is_snoozed(now))
        .collect()
}

/// The id of the tip to highlight as "next up", if any.
pub fn next_up(tips: &[Tip], now: NaiveDateTime) -> Option<String> {
    visible(tips, now)
        .into_iter()
        .max_by(|a, b| {
            score(a, now)
                .cmp(&score(b, now))
                .then_with(|| b.due_at.cmp(&a.due_at))
                .then_with(|| b.front.title.cmp(&a.front.title))
        })
        .map(|t| t.id.clone())
}

/// Board order: highest score first, stable by title.
pub fn ordered(tips: &[Tip], now: NaiveDateTime) -> Vec<&Tip> {
    let mut v = visible(tips, now);
    v.sort_by(|a, b| {
        score(b, now)
            .cmp(&score(a, now))
            .then_with(|| a.due_at.cmp(&b.due_at))
            .then_with(|| a.front.title.cmp(&b.front.title))
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tip::FrontMatter;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn tip(id: &str, kind: Kind, priority: Priority, due: Option<&str>) -> Tip {
        let front = FrontMatter {
            title: id.to_string(),
            kind,
            priority,
            due: due.map(str::to_string),
            ..Default::default()
        };
        Tip::from_parts(&format!("tips/{id}.md"), None, front, String::new())
    }

    #[test]
    fn overdue_beats_high_priority() {
        let now = dt("2026-10-01 12:00");
        let tips = vec![
            tip("high-later", Kind::Task, Priority::High, Some("2026-12-01")),
            tip("overdue-low", Kind::Note, Priority::Low, Some("2026-09-30")),
        ];
        assert_eq!(next_up(&tips, now).as_deref(), Some("overdue-low"));
    }

    #[test]
    fn priority_breaks_ties_then_kind_then_due() {
        let now = dt("2026-10-01 12:00");
        let tips = vec![
            tip("a-normal", Kind::Task, Priority::Normal, None),
            tip("b-high", Kind::Task, Priority::High, None),
            tip("c-high-deadline", Kind::Deadline, Priority::High, None),
        ];
        assert_eq!(next_up(&tips, now).as_deref(), Some("c-high-deadline"));
        let tips = vec![
            tip("later", Kind::Task, Priority::Normal, Some("2026-10-05")),
            tip("sooner", Kind::Task, Priority::Normal, Some("2026-10-04")),
        ];
        assert_eq!(next_up(&tips, now).as_deref(), Some("sooner"));
    }

    #[test]
    fn done_and_snoozed_are_excluded() {
        let now = dt("2026-10-01 12:00");
        let done = tip("done", Kind::Task, Priority::High, Some("2026-09-01")).complete(now);
        let snoozed = tip("snoozed", Kind::Task, Priority::High, Some("2026-09-01"))
            .snooze_until(dt("2026-10-01 13:00"));
        let plain = tip("plain", Kind::Note, Priority::Low, None);
        let tips = vec![done, snoozed, plain];
        assert_eq!(next_up(&tips, now).as_deref(), Some("plain"));
        assert_eq!(ordered(&tips, now).len(), 1);
        assert_eq!(next_up(&[], now), None);
    }

    #[test]
    fn ordered_is_descending_by_score() {
        let now = dt("2026-10-01 12:00");
        let tips = vec![
            tip("note", Kind::Note, Priority::Low, None),
            tip("soon", Kind::Task, Priority::Normal, Some("2026-10-05")),
            tip(
                "critical",
                Kind::Deadline,
                Priority::Normal,
                Some("2026-10-02"),
            ),
        ];
        let ids: Vec<&str> = ordered(&tips, now).iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["critical", "soon", "note"]);
    }
}
