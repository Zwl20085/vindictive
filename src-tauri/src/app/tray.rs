//! System tray icon and menu.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use super::state::AppState;
use super::{updater, windows};

const ID_SHOW: &str = "show";
const ID_SYNC: &str = "sync";
const ID_CAPTURE: &str = "capture";
const ID_SETTINGS: &str = "settings";
const ID_UPDATE: &str = "update";
const ID_QUIT: &str = "quit";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, ID_SHOW, "Show / hide board", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, ID_SYNC, "Reload tips folder", true, None::<&str>)?;
    let capture = MenuItem::with_id(app, ID_CAPTURE, "Quick capture", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, ID_SETTINGS, "Settings…", true, None::<&str>)?;
    let update = MenuItem::with_id(app, ID_UPDATE, "Check for updates…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit Vindictive", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &sync,
            &capture,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &update,
            &quit,
        ],
    )?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("window icon".into()))?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Vindictive")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Err(e) = windows::toggle_main(tray.app_handle()) {
                    log::warn!("{e}");
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    let result = match id {
        ID_SHOW => windows::toggle_main(app),
        ID_CAPTURE => windows::show_capture(app),
        ID_SETTINGS => windows::open_settings(app),
        ID_SYNC => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                if let Err(e) = state.sync(&app).await {
                    log::warn!("manual sync: {e}");
                }
            });
            Ok(())
        }
        ID_UPDATE => {
            updater::check_and_install(app);
            Ok(())
        }
        ID_QUIT => {
            app.exit(0);
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(e) = result {
        log::warn!("tray action {id}: {e}");
    }
}
