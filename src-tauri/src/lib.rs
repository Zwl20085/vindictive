//! Vindictive: a tile board of what to do next.

pub mod app;
pub mod core;
pub mod sync;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use app::settings::Settings;
use app::state::AppState;
use app::storage::Storage;
use app::{commands, hotkeys, peek, scheduler, tray, update_check, windows};
use sync::folder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch exits at once and the
        // running copy shows its board instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Err(e) = windows::show_main(app) {
                log::warn!("second launch: {e}");
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        hotkeys::on_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            // Managed first: the board window may ask for state right away.
            app.manage(AppState::load(Storage::new(data_dir)));
            let state = app.state::<AppState>();
            if !state.settings().is_configured() {
                choose_default_folder(app.handle(), &state);
            }
            let settings = state.settings();

            let handle = app.handle().clone();
            tray::build(&handle)?;
            windows::apply_initial(&handle, &settings);
            scheduler::start(handle.clone());
            update_check::start(handle);
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (windows::MAIN, WindowEvent::CloseRequested { api, .. }) => {
                // Closing the board hides it; the tray keeps the app alive.
                api.prevent_close();
                peek::forget();
                let _ = window.hide();
            }
            (windows::MAIN, WindowEvent::Focused(false)) => {
                peek::on_blur(window.app_handle());
            }
            (windows::CAPTURE, WindowEvent::Focused(false)) => {
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::sync_now,
            commands::mark_done,
            commands::reopen,
            commands::snooze,
            commands::delete_tip,
            commands::edit_local,
            commands::reveal_tip,
            commands::open_folder,
            commands::pick_folder,
            commands::set_color,
            commands::set_size,
            commands::set_order,
            commands::update_tip,
            commands::fit_window,
            commands::create_tip,
            commands::save_settings,
            commands::fetch_image,
            commands::enrich_tip,
            commands::fetch_weather,
            commands::show_capture,
            commands::hide_capture,
            commands::dock_window,
            commands::frontend_log,
            commands::quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vindictive");
}

/// First start (or first start after the GitHub era): use `Vindictive` in
/// OneDrive, or in Documents without OneDrive, so there is nothing to set up.
fn choose_default_folder(app: &tauri::AppHandle, state: &AppState) {
    let documents = app
        .path()
        .document_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let target = folder::default_folder(folder::onedrive_root().as_deref(), &documents);
    match commands::ensure_folder(&target) {
        Ok(path) => {
            let settings = Settings {
                folder: path.display().to_string(),
                ..state.settings()
            };
            match state.set_settings(settings) {
                Ok(()) => log::info!("tips folder: {}", path.display()),
                Err(e) => log::warn!("tips folder not saved: {e}"),
            }
        }
        Err(e) => log::warn!("{e}"),
    }
}
