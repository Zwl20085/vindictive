//! Tauri application layer: state, commands, background scheduler, tray,
//! windows. Everything with side effects lives here.

pub mod board;
pub mod commands;
pub mod editor;
pub mod foreground;
pub mod hotkeys;
pub mod notify;
pub mod peek;
pub mod scheduler;
pub mod settings;
pub mod state;
pub mod storage;
pub mod tray;
pub mod update_check;
pub mod updater;
pub mod windows;
