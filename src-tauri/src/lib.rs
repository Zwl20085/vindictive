//! Vindictive: an always-on-top tile board of what to do next.

pub mod app;
pub mod core;
pub mod sync;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use app::state::AppState;
use app::storage::Storage;
use app::{commands, editor, scheduler, tray, windows};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Err(e) = windows::show_capture(app) {
                            log::warn!("capture: {e}");
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::load(Storage::new(data_dir));
            let settings = state.settings();
            app.manage(state);

            let handle = app.handle().clone();
            tray::build(&handle)?;
            windows::apply_initial(&handle, &settings);
            scheduler::start(handle.clone());
            editor::start(handle);
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (windows::MAIN, WindowEvent::CloseRequested { api, .. }) => {
                // Closing the board hides it; the tray keeps the app alive.
                api.prevent_close();
                let _ = window.hide();
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
            commands::set_color,
            commands::set_size,
            commands::push_now,
            commands::fit_window,
            commands::create_tip,
            commands::save_settings,
            commands::set_token,
            commands::clear_token,
            commands::test_connection,
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
