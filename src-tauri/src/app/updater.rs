//! Self-update from the latest GitHub release.
//!
//! The release workflow publishes a signed installer plus `latest.json`; the
//! updater plugin compares versions, checks the signature against the public
//! key in `tauri.conf.json`, runs the installer and the app restarts.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use super::notify;

/// Set while a check or install runs, so repeated clicks do nothing.
static BUSY: AtomicBool = AtomicBool::new(false);

/// Holds the "checking for updates" flag; dropping it clears the flag, so a
/// failed or panicking check never blocks the next one.
pub struct Busy(());

impl Busy {
    /// `None` while another check (manual or background) is running.
    pub fn start() -> Option<Busy> {
        (!BUSY.swap(true, Ordering::SeqCst)).then_some(Busy(()))
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// Check GitHub for a newer release and, if there is one, install it and
/// restart. Every outcome is reported with a toast.
pub fn check_and_install(app: &AppHandle) {
    let Some(busy) = Busy::start() else {
        notify::show(app, "Vindictive", "Already checking for updates…");
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _busy = busy;
        if let Err(e) = run(&app).await {
            log::warn!("update: {e}");
            notify::show(&app, "Update failed", &e);
        }
    });
}

async fn run(app: &AppHandle) -> Result<(), String> {
    let current = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| match e {
            // GitHub answered, but the latest release carries no latest.json.
            tauri_plugin_updater::Error::ReleaseNotFound => {
                "the latest GitHub release has no update file yet".to_string()
            }
            other => format!("could not reach GitHub: {other}"),
        })?;
    let Some(update) = update else {
        notify::show(
            app,
            "Vindictive is up to date",
            &format!("Version {current}"),
        );
        return Ok(());
    };
    notify::show(
        app,
        "Updating Vindictive",
        &format!(
            "Downloading {} (you have {current}); the app restarts when done.",
            update.version
        ),
    );
    update
        .download_and_install(|_, _| {}, || log::info!("update: download finished"))
        .await
        .map_err(|e| format!("install failed: {e}"))?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_check_at_a_time() {
        let first = Busy::start().expect("idle at first");
        assert!(Busy::start().is_none());
        drop(first);
        assert!(Busy::start().is_some());
    }
}
