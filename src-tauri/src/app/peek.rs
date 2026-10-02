//! "Peek" at the board: the board hotkey and a left click on the tray icon.
//!
//! With "always on bottom" the board sits under every window, so showing it
//! is not enough: peeking lifts it above the others and focuses it, and the
//! next peek (or the board losing focus) puts it back on the bottom layer.
//! In the other layer modes peeking simply shows or hides the board.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::foreground;
use super::settings::Settings;
use super::state::AppState;
use super::windows;

/// True while the board is lifted above its usual bottom layer.
static PEEKING: AtomicBool = AtomicBool::new(false);
/// The window that was in front when the board was lifted (`0` = none).
static PREVIOUS: AtomicIsize = AtomicIsize::new(0);
/// When losing focus last ended a peek.
static BLUR_DROPPED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// A click on the tray icon first takes focus from the board (which ends the
/// peek) and then asks to peek. Within this window that request is the same
/// gesture, so it must not lift the board again.
const BLUR_GRACE: Duration = Duration::from_millis(400);
/// When the board was last lifted.
static LIFTED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Taking focus for a board that was in the background makes Windows report
/// a brief loss of focus first. A blur this soon after a lift is that
/// hand-over, not the user leaving, and must not end the peek.
const LIFT_GRACE: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeekStep {
    /// Hidden: show it (lifted, when it lives on the bottom layer).
    Show { lift: bool },
    /// On the bottom layer: lift it above everything and focus it.
    Lift,
    /// Lifted: back to the layer the settings ask for.
    Drop,
    /// Normal or always-on-top board: hide it.
    Hide,
}

/// Pure decision for one peek.
pub fn peek_step(visible: bool, on_bottom: bool, peeking: bool) -> PeekStep {
    match (visible, on_bottom, peeking) {
        (false, _, _) => PeekStep::Show { lift: on_bottom },
        (true, true, false) => PeekStep::Lift,
        (true, true, true) => PeekStep::Drop,
        (true, false, _) => PeekStep::Hide,
    }
}

/// True when a lift request is only the tail of the click that just ended
/// a peek by taking focus away.
pub fn already_dropped(step: PeekStep, since_blur_drop: Option<Duration>) -> bool {
    step == PeekStep::Lift && since_blur_drop.is_some_and(|d| d < BLUR_GRACE)
}

/// True when a loss of focus means the user left the lifted board.
pub fn blur_ends_peek(since_lift: Option<Duration>) -> bool {
    !since_lift.is_some_and(|d| d < LIFT_GRACE)
}

/// Peek once: show, lift, drop back or hide, depending on the board.
pub fn toggle(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let w = app
        .get_webview_window(windows::MAIN)
        .ok_or("board window missing")?;
    let visible = w.is_visible().map_err(|e| e.to_string())?;
    let step = peek_step(
        visible,
        settings.always_on_bottom,
        PEEKING.load(Ordering::SeqCst),
    );
    let since_blur_drop = BLUR_DROPPED_AT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .map(|at| at.elapsed());
    if already_dropped(step, since_blur_drop) {
        return Ok(());
    }
    match step {
        PeekStep::Show { lift: false } => windows::show_main(app),
        PeekStep::Show { lift: true } | PeekStep::Lift => lift(app),
        PeekStep::Drop => {
            let focused = w.is_focused().unwrap_or(false);
            drop_back(app, settings);
            // The board keeps the keyboard when it sinks; hand it back.
            if focused {
                foreground::restore(PREVIOUS.swap(0, Ordering::SeqCst));
            }
            Ok(())
        }
        PeekStep::Hide => {
            forget();
            w.hide().map_err(|e| e.to_string())
        }
    }
}

fn lift(app: &AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window(windows::MAIN)
        .ok_or("board window missing")?;
    if !w.is_focused().unwrap_or(false) {
        PREVIOUS.store(foreground::current(), Ordering::SeqCst);
    }
    PEEKING.store(true, Ordering::SeqCst);
    *LIFTED_AT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    // Order matters: leave the bottom layer, take focus, and only then pin
    // on top. Pinning before the focus hand-over does not stick when another
    // window is in front.
    if let Err(e) = w.set_always_on_bottom(false) {
        log::warn!("peek: leaving the bottom layer failed: {e}");
    }
    let shown = windows::show_main(app);
    if let Err(e) = w.set_always_on_top(true) {
        log::warn!("peek: pinning on top failed: {e}");
    }
    log::info!("peek: lifted");
    shown
}

/// Put a lifted board back on the layer from `settings`. No-op otherwise.
pub fn drop_back(app: &AppHandle, settings: &Settings) {
    if !PEEKING.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window(windows::MAIN) {
        windows::apply_layer(&w, settings.always_on_top, settings.always_on_bottom);
    }
    log::info!("peek: dropped back");
}

/// The board lost focus: a peek ends there.
pub fn on_blur(app: &AppHandle) {
    let since_lift = LIFTED_AT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .map(|at| at.elapsed());
    if PEEKING.load(Ordering::SeqCst) && blur_ends_peek(since_lift) {
        drop_back(app, &AppState::from_app(app).settings());
        *BLUR_DROPPED_AT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    }
}

/// The layer settings changed: whatever was lifted now follows them.
pub fn forget() {
    PEEKING.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_boards_are_shown_and_lifted_when_on_bottom() {
        assert_eq!(peek_step(false, true, false), PeekStep::Show { lift: true });
        assert_eq!(
            peek_step(false, false, false),
            PeekStep::Show { lift: false }
        );
        // A stale peek flag on a hidden board still just shows it.
        assert_eq!(peek_step(false, true, true), PeekStep::Show { lift: true });
    }

    #[test]
    fn bottom_boards_lift_and_drop_back() {
        assert_eq!(peek_step(true, true, false), PeekStep::Lift);
        assert_eq!(peek_step(true, true, true), PeekStep::Drop);
    }

    #[test]
    fn a_tray_click_that_ended_a_peek_does_not_lift_again() {
        let just_now = Some(Duration::from_millis(50));
        let long_ago = Some(Duration::from_secs(5));
        assert!(already_dropped(PeekStep::Lift, just_now));
        assert!(!already_dropped(PeekStep::Lift, long_ago));
        assert!(!already_dropped(PeekStep::Lift, None));
        assert!(!already_dropped(PeekStep::Drop, just_now));
        assert!(!already_dropped(PeekStep::Hide, just_now));
    }

    #[test]
    fn the_focus_hand_over_after_a_lift_does_not_end_the_peek() {
        assert!(!blur_ends_peek(Some(Duration::from_millis(40))));
        assert!(blur_ends_peek(Some(Duration::from_secs(3))));
        assert!(blur_ends_peek(None));
    }

    #[test]
    fn other_boards_toggle_like_the_tray() {
        assert_eq!(peek_step(true, false, false), PeekStep::Hide);
        assert_eq!(peek_step(true, false, true), PeekStep::Hide);
    }
}
