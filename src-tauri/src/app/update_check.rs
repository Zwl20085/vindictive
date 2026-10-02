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
        loop {
            if AppState::from_app(&app).settings().auto_update_check {
                check_quietly(&app).await;
            } else {
                log::info!("update check: switched off in settings");
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

async fn check_quietly(app: &AppHandle) {
    // A manual check or install is running: it reports on its own.
    let Some(_busy) = updater::Busy::start() else {
        return;
    };
    let current = app.package_info().version.to_string();
    let result = match app.updater() {
        Ok(updater) => updater.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    match result {
        Ok(Some(update)) => announce(app, &update.version, &current),
        Ok(None) => log::info!("update check: {current} is the latest"),
        Err(e) => log::info!("update check skipped: {e}"),
    }
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
}
