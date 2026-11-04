//! Quiet background update check: shortly after start and then once a day.
//!
//! It never installs anything (a surprise restart of a desktop widget is not
//! acceptable). When a newer release exists it shows one toast per version
//! per run, pointing at the tray's "Check for updates…", which installs.
//! No update, or no network, is only a log line.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use super::state::AppState;
use super::{notify, updater};

/// Wait this long after start so the check never competes with startup.
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(45);
/// Then check again this often.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// After a failed check (no network at start-up, say), try again this soon...
pub const RETRY_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// ...this many times in a row before falling back to the daily rhythm.
pub const MAX_RETRIES: u32 = 3;

/// Wait before the next round, given how many checks in a row have failed.
pub fn next_delay(failures: u32) -> Duration {
    if (1..=MAX_RETRIES).contains(&failures) {
        RETRY_INTERVAL
    } else {
        CHECK_INTERVAL
    }
}

/// Versions already announced in this run.
static ANNOUNCED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Announce `available` unless it is the running version or was announced
/// already. The updater plugin only reports releases newer than `current`.
pub fn should_notify(available: &str, current: &str, announced: &BTreeSet<String>) -> bool {
    let available = available.trim();
    !available.is_empty() && available != current.trim() && !announced.contains(available)
}

/// The toast for a newer version: `(title, body)`.
pub fn toast_text(version: &str) -> (String, String) {
    (
        format!("Vindictive {version} is available"),
        "Right-click the tray icon → Check for updates".to_string(),
    )
}

/// Start the background loop. Each round reads the `auto_update_check`
/// setting afresh, so switching it off takes effect at the next round.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        let mut failures = 0u32;
        loop {
            if AppState::from_app(&app).settings().auto_update_check {
                failures = match check_quietly(&app).await {
                    Ok(()) => 0,
                    // Past the retries: back to daily, and a fresh streak.
                    Err(()) if failures >= MAX_RETRIES => 0,
                    Err(()) => failures + 1,
                };
            } else {
                log::info!("update check: switched off in settings");
                failures = 0;
            }
            tokio::time::sleep(next_delay(failures)).await;
        }
    });
}

/// `Err` when the check itself failed (network, endpoint), so it is retried
/// sooner than the daily round.
async fn check_quietly(app: &AppHandle) -> Result<(), ()> {
    // A manual check or install is running: it reports on its own.
    let Some(_busy) = updater::Busy::start() else {
        return Ok(());
    };
    let current = app.package_info().version.to_string();
    let result = match app.updater() {
        Ok(updater) => updater.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    match result {
        Ok(Some(update)) => announce(app, &update.version, &current),
        Ok(None) => log::info!("update check: {current} is the latest"),
        Err(e) => {
            log::warn!("update check failed: {e}");
            return Err(());
        }
    }
    Ok(())
}

fn announce(app: &AppHandle, version: &str, current: &str) {
    let first_time = {
        let mut announced = ANNOUNCED.lock().unwrap_or_else(|p| p.into_inner());
        let fresh = should_notify(version, current, &announced);
        if fresh {
            announced.insert(version.trim().to_string());
        }
        fresh
    };
    if first_time {
        log::info!("update check: {version} is available (running {current})");
        let (title, body) = toast_text(version);
        notify::show(app, &title, &body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announces_each_new_version_once() {
        let mut announced = BTreeSet::new();
        assert!(should_notify("0.4.1", "0.4.0", &announced));
        announced.insert("0.4.1".to_string());
        assert!(
            !should_notify("0.4.1", "0.4.0", &announced),
            "once per version"
        );
        assert!(
            should_notify("0.4.2", "0.4.0", &announced),
            "a later release is news again"
        );
    }

    #[test]
    fn never_announces_the_running_version_or_nothing() {
        let none = BTreeSet::new();
        assert!(!should_notify("0.4.0", "0.4.0", &none));
        assert!(!should_notify(" 0.4.0 ", "0.4.0", &none));
        assert!(!should_notify("", "0.4.0", &none));
    }

    #[test]
    fn toast_names_the_version_and_the_way_to_install() {
        let (title, body) = toast_text("0.4.1");
        assert_eq!(title, "Vindictive 0.4.1 is available");
        assert!(body.contains("Check for updates"));
    }

    #[test]
    fn checks_are_spaced_out() {
        assert!(FIRST_CHECK_DELAY >= Duration::from_secs(30));
        assert!(FIRST_CHECK_DELAY <= Duration::from_secs(60));
        assert_eq!(CHECK_INTERVAL, Duration::from_secs(86_400));
    }

    #[test]
    fn failed_checks_retry_hourly_a_few_times() {
        assert_eq!(next_delay(0), CHECK_INTERVAL);
        for n in 1..=MAX_RETRIES {
            assert_eq!(next_delay(n), RETRY_INTERVAL, "retry {n}");
        }
        assert_eq!(next_delay(MAX_RETRIES + 1), CHECK_INTERVAL);
        assert_eq!(RETRY_INTERVAL, Duration::from_secs(3600));
    }
}
