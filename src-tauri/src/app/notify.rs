//! Native toast notifications and the pure logic deciding which reminders
//! are due.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDateTime;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::core::deadline::{urgency, Urgency};
use crate::core::nextup::visible;
use crate::core::tip::{Kind, Tip};

/// Reminders fired more than this long ago are dropped from the fired set.
const RETENTION_DAYS: i64 = 60;
/// A reminder older than this when first seen is *stale*: the app was off or
/// asleep. Stale reminders are not toasted one by one but summed up in one
/// digest toast.
const STALE_HOURS: i64 = 12;
/// Reminders older than this are dropped without a word. It must stay well
/// inside `RETENTION_DAYS`, or a pruned key would make its reminder due again.
const DIGEST_WINDOW_DAYS: i64 = 7;
/// More fresh reminders than this in one tick are also folded into a digest.
pub const MAX_SEPARATE_TOASTS: usize = 5;
/// Titles named in a digest body.
const DIGEST_TITLES: usize = 3;
/// A toast that fails this many times is recorded as fired anyway, so a
/// broken notification service cannot make the app retry forever.
pub const MAX_TOAST_ATTEMPTS: u32 = 3;
/// Timestamp format at the end of every fired key.
const KEY_TIME: &str = "%Y-%m-%dT%H:%M";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub key: String,
    pub title: String,
    pub body: String,
    /// Due more than `STALE_HOURS` ago (the app was not running then).
    pub stale: bool,
}

/// One toast to show and the reminder keys it settles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub title: String,
    pub body: String,
    pub keys: Vec<String>,
}

/// Compute every reminder that should fire at `now` and has not fired yet,
/// including stale ones from the last `DIGEST_WINDOW_DAYS`.
pub fn due_reminders(tips: &[Tip], fired: &BTreeSet<String>, now: NaiveDateTime) -> Vec<Toast> {
    let stale_before = now - chrono::Duration::hours(STALE_HOURS);
    let too_old = now - chrono::Duration::days(DIGEST_WINDOW_DAYS);
    let mut out = Vec::new();
    for tip in visible(tips, now) {
        for at in &tip.remind_at {
            if *at <= now && *at > too_old {
                let key = format!("{}@{}", tip.id, at.format(KEY_TIME));
                if !fired.contains(&key) {
                    out.push(Toast {
                        key,
                        title: tip.display_title().to_string(),
                        body: reminder_body(tip, now),
                        stale: *at <= stale_before,
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
            if due > too_old && !fired.contains(&key) {
                out.push(Toast {
                    key,
                    title: format!("Overdue: {}", tip.display_title()),
                    body: reminder_body(tip, now),
                    stale: due <= stale_before,
                });
            }
        }
    }
    out
}

/// How to show `toasts`: one toast each, or (when any is stale, or there are
/// more than `MAX_SEPARATE_TOASTS`) a single digest settling all of them.
pub fn plan_delivery(toasts: Vec<Toast>, zh: bool) -> Vec<Delivery> {
    if toasts.len() <= MAX_SEPARATE_TOASTS && !toasts.iter().any(|t| t.stale) {
        return toasts
            .into_iter()
            .map(|t| Delivery {
                title: t.title,
                body: t.body,
                keys: vec![t.key],
            })
            .collect();
    }
    if toasts.is_empty() {
        return Vec::new();
    }
    let titles: Vec<&str> = toasts.iter().map(|t| t.title.as_str()).collect();
    let (title, body) = digest_text(&titles, zh);
    vec![Delivery {
        title,
        body,
        keys: toasts.into_iter().map(|t| t.key).collect(),
    }]
}

/// `(title, body)` of the digest toast for reminders with these titles.
pub fn digest_text(titles: &[&str], zh: bool) -> (String, String) {
    let n = titles.len();
    let shown = titles[..n.min(DIGEST_TITLES)].join(" · ");
    let more = n.saturating_sub(DIGEST_TITLES);
    if zh {
        let body = if more > 0 {
            format!("{shown} · 另有 {more} 条")
        } else {
            shown
        };
        (format!("离开期间有 {n} 条提醒"), body)
    } else {
        let body = if more > 0 {
            format!("{shown} · +{more} more")
        } else {
            shown
        };
        let noun = if n == 1 { "reminder" } else { "reminders" };
        (format!("{n} {noun} while you were away"), body)
    }
}

/// Record the outcome of showing a toast for `keys` and return the keys to
/// mark as fired: all of them on success, or once a key has failed
/// `MAX_TOAST_ATTEMPTS` times; otherwise none, so the next tick retries.
pub fn settle(failures: &mut BTreeMap<String, u32>, keys: Vec<String>, shown: bool) -> Vec<String> {
    keys.into_iter()
        .filter(|key| {
            if shown {
                failures.remove(key);
                return true;
            }
            let count = failures.entry(key.clone()).or_insert(0);
            *count += 1;
            let give_up = *count >= MAX_TOAST_ATTEMPTS;
            if give_up {
                failures.remove(key);
            }
            give_up
        })
        .collect()
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

/// Show a toast; failures are logged and otherwise ignored.
pub fn show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(e) = try_show(app, title, body) {
        log::error!("toast failed ({title}): {e}");
    }
}

/// Show a toast and report whether it worked.
pub fn try_show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| e.to_string())
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
        assert!(!toasts[0].stale);
        fired.insert(toasts[0].key.clone());
        assert!(due_reminders(std::slice::from_ref(&t), &fired, now).is_empty());
        // Seen for the first time two days later: stale, so digest material.
        let late = due_reminders(
            std::slice::from_ref(&t),
            &BTreeSet::new(),
            dt("2026-10-04 09:00"),
        );
        assert!(!late.is_empty());
        assert!(late.iter().all(|t| t.stale));
        // A month later it is dropped without a word.
        assert!(due_reminders(
            std::slice::from_ref(&t),
            &BTreeSet::new(),
            dt("2026-11-04 09:00")
        )
        .is_empty());
    }

    fn toast(key: &str, stale: bool) -> Toast {
        Toast {
            key: key.into(),
            title: format!("T {key}"),
            body: "b".into(),
            stale,
        }
    }

    #[test]
    fn few_fresh_reminders_are_toasted_one_by_one() {
        let plan = plan_delivery(vec![toast("a", false), toast("b", false)], false);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].keys, vec!["a"]);
        assert_eq!(plan[0].title, "T a");
        assert!(plan_delivery(Vec::new(), false).is_empty());
    }

    #[test]
    fn stale_or_many_reminders_become_one_digest() {
        let plan = plan_delivery(vec![toast("a", false), toast("b", true)], false);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].keys, vec!["a", "b"]);
        assert_eq!(plan[0].title, "2 reminders while you were away");
        assert_eq!(plan[0].body, "T a · T b");

        let many: Vec<Toast> = (0..=MAX_SEPARATE_TOASTS)
            .map(|i| toast(&i.to_string(), false))
            .collect();
        let plan = plan_delivery(many, false);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].keys.len(), MAX_SEPARATE_TOASTS + 1);
        assert_eq!(plan[0].body, "T 0 · T 1 · T 2 · +3 more");

        let plan = plan_delivery(vec![toast("a", true)], true);
        assert_eq!(plan[0].title, "离开期间有 1 条提醒");
        let (title, _) = digest_text(&["x"], false);
        assert_eq!(title, "1 reminder while you were away");
        let (_, body) = digest_text(&["a", "b", "c", "d"], true);
        assert_eq!(body, "a · b · c · 另有 1 条");
    }

    #[test]
    fn failed_toasts_are_retried_then_given_up() {
        let mut failures = BTreeMap::new();
        let keys = || vec!["a".to_string()];
        for _ in 1..MAX_TOAST_ATTEMPTS {
            assert!(settle(&mut failures, keys(), false).is_empty());
        }
        assert_eq!(settle(&mut failures, keys(), false), keys());
        assert!(failures.is_empty(), "a given-up key is forgotten");
        assert!(settle(&mut failures, keys(), false).is_empty());
        assert_eq!(settle(&mut failures, keys(), true), keys());
        assert!(failures.is_empty(), "success clears the count");
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
