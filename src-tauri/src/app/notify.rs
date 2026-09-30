//! Native toast notifications and the pure logic deciding which reminders
//! are due.

use std::collections::BTreeSet;

use chrono::NaiveDateTime;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::core::deadline::{urgency, Urgency};
use crate::core::nextup::visible;
use crate::core::tip::{Kind, Tip};

/// Reminders fired more than this long ago are dropped from the fired set.
const RETENTION_DAYS: i64 = 60;
/// A reminder older than this at the time it is first seen is not re-fired.
const STALE_HOURS: i64 = 12;
/// Timestamp format at the end of every fired key.
const KEY_TIME: &str = "%Y-%m-%dT%H:%M";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub key: String,
    pub title: String,
    pub body: String,
}

/// Compute every reminder that should fire at `now` and has not fired yet.
pub fn due_reminders(tips: &[Tip], fired: &BTreeSet<String>, now: NaiveDateTime) -> Vec<Toast> {
    let stale_before = now - chrono::Duration::hours(STALE_HOURS);
    let mut out = Vec::new();
    for tip in visible(tips, now) {
        for at in &tip.remind_at {
            if *at <= now && *at > stale_before {
                let key = format!("{}@{}", tip.id, at.format(KEY_TIME));
                if !fired.contains(&key) {
                    out.push(Toast {
                        key,
                        title: tip.display_title().to_string(),
                        body: reminder_body(tip, now),
                    });
                }
            }
        }
        if let Some(due) = tip.due_at.filter(|_| {
            matches!(tip.front.kind, Kind::Deadline | Kind::Task | Kind::Event)
                && urgency(tip.due_at, now) == Urgency::Overdue
        }) {
            // One key per due date, so a recurring or rescheduled tip warns again.
            let key = format!("{}@overdue@{}", tip.id, due.format(KEY_TIME));
            if due > stale_before && !fired.contains(&key) {
                out.push(Toast {
                    key,
                    title: format!("Overdue: {}", tip.display_title()),
                    body: reminder_body(tip, now),
                });
            }
        }
    }
    out
}

fn reminder_body(tip: &Tip, now: NaiveDateTime) -> String {
    let mut parts = Vec::new();
    if let Some(due) = tip.due_at {
        parts.push(format!("Due {}", human_delta(due, now)));
    }
    if let Some(loc) = &tip.front.location {
        parts.push(loc.clone());
    }
    if parts.is_empty() {
        first_line(&tip.body)
    } else {
        parts.join(" · ")
    }
}

fn first_line(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// `in 2d 4h`, `in 45m`, `2h ago`.
pub fn human_delta(target: NaiveDateTime, now: NaiveDateTime) -> String {
    let delta = target - now;
    let mins = delta.num_minutes().abs();
    let text = if mins >= 60 * 24 {
        let d = mins / (60 * 24);
        let h = (mins % (60 * 24)) / 60;
        if h > 0 {
            format!("{d}d {h}h")
        } else {
            format!("{d}d")
        }
    } else if mins >= 60 {
        format!("{}h", mins / 60)
    } else {
        format!("{mins}m")
    };
    if delta.num_minutes() >= 0 {
        format!("in {text}")
    } else {
        format!("{text} ago")
    }
}

/// Drop fired keys older than the retention window so the file stays small.
/// Keys without a timestamp (the old `id@overdue` form) are dropped too.
pub fn prune_fired(fired: &BTreeSet<String>, now: NaiveDateTime) -> BTreeSet<String> {
    let cutoff = now - chrono::Duration::days(RETENTION_DAYS);
    fired
        .iter()
        .filter(|k| {
            k.rsplit_once('@')
                .and_then(|(_, t)| parse_key_time(t))
                .is_some_and(|t| t >= cutoff)
        })
        .cloned()
        .collect()
}

fn parse_key_time(t: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(t, KEY_TIME).ok()
}

pub fn show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::error!("toast failed ({title}): {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn tip(text: &str) -> Tip {
        Tip::parse("tips/t.md", None, text).unwrap()
    }

    #[test]
    fn fires_once_and_skips_stale() {
        let t = tip("---\ntitle: Coil check\nlocation: Lab 302\ndue: 2026-10-02 12:00\nremind: [2026-10-01 09:00, 2026-10-02 11:00]\n---\n");
        let now = dt("2026-10-01 09:05");
        let mut fired = BTreeSet::new();
        let toasts = due_reminders(std::slice::from_ref(&t), &fired, now);
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].key, "t@2026-10-01T09:00");
        assert_eq!(toasts[0].title, "Coil check");
        assert_eq!(toasts[0].body, "Due in 1d 2h · Lab 302");
        fired.insert(toasts[0].key.clone());
        assert!(due_reminders(std::slice::from_ref(&t), &fired, now).is_empty());
        // Seen for the first time two days later: too stale to fire.
        assert!(due_reminders(
            std::slice::from_ref(&t),
            &BTreeSet::new(),
            dt("2026-10-04 09:00")
        )
        .is_empty());
    }

    #[test]
    fn overdue_toast_for_actionable_kinds_only() {
        let deadline =
            tip("---\ntitle: D\nkind: deadline\ndue: 2026-10-01 10:00\n---\nfirst line\n");
        let note = tip("---\ntitle: N\nkind: note\ndue: 2026-10-01 10:00\n---\n");
        let now = dt("2026-10-01 10:30");
        let toasts = due_reminders(&[deadline, note], &BTreeSet::new(), now);
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].key, "t@overdue@2026-10-01T10:00");
        assert_eq!(toasts[0].title, "Overdue: D");
        assert_eq!(toasts[0].body, "Due 30m ago");
    }

    #[test]
    fn overdue_fires_again_for_a_new_due_date() {
        let first = tip("---
title: D
kind: deadline
due: 2026-10-01 10:00
---
");
        let fired: BTreeSet<String> =
            due_reminders(&[first], &BTreeSet::new(), dt("2026-10-01 10:30"))
                .into_iter()
                .map(|t| t.key)
                .collect();
        let moved = tip("---
title: D
kind: deadline
due: 2026-10-08 10:00
---
");
        let again = due_reminders(&[moved], &fired, dt("2026-10-08 10:30"));
        assert_eq!(again.len(), 1);
    }

    #[test]
    fn snoozed_or_done_tips_are_silent() {
        let t = tip("---\ntitle: S\nremind: [2026-10-01 09:00]\n---\n");
        let now = dt("2026-10-01 09:05");
        let snoozed = t.snooze_until(dt("2026-10-01 10:00"));
        assert!(due_reminders(&[snoozed], &BTreeSet::new(), now).is_empty());
        let done = t.complete(now);
        assert!(due_reminders(&[done], &BTreeSet::new(), now).is_empty());
    }

    #[test]
    fn human_deltas() {
        let now = dt("2026-10-01 12:00");
        assert_eq!(human_delta(dt("2026-10-03 16:00"), now), "in 2d 4h");
        assert_eq!(human_delta(dt("2026-10-03 12:00"), now), "in 2d");
        assert_eq!(human_delta(dt("2026-10-01 15:30"), now), "in 3h");
        assert_eq!(human_delta(dt("2026-10-01 12:45"), now), "in 45m");
        assert_eq!(human_delta(dt("2026-10-01 10:00"), now), "2h ago");
    }

    #[test]
    fn prunes_old_keys() {
        let now = dt("2026-10-01 12:00");
        let fired: BTreeSet<String> = [
            "a@2026-01-01T09:00",
            "b@2026-09-30T09:00",
            "c@overdue",
            "d@overdue@2026-09-30T09:00",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let pruned = prune_fired(&fired, now);
        assert!(!pruned.contains("a@2026-01-01T09:00"));
        assert!(pruned.contains("b@2026-09-30T09:00"));
        assert!(!pruned.contains("c@overdue"), "legacy key without a time");
        assert!(pruned.contains("d@overdue@2026-09-30T09:00"));
    }
}
