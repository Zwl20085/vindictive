//! "Peek" at the board: the board hotkey and a left click on the tray icon.
//!
//! With "always on bottom" the board sits under every window, so showing it
//! is not enough: peeking lifts it above the others and focuses it, and the
//! next peek (or the user switching to another app) puts it back on the
//! bottom layer. In the other layer modes peeking simply shows or hides the
//! board.
//!
//! A loss of focus alone is not "the user left": dragging the board by its
//! title strip, resizing it by an edge or opening a native menu or dialog
//! from it all make the board report a blur while this app still owns the
//! foreground. Dropping there would sink the board mid-drag (the bottom
//! layer is re-imposed on every position change), so a blur only schedules
//! a check of who really is in front.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
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
/// gesture, so it must not lift the board again. Measured from the drop,
/// which now lands one `BLUR_CHECK_DELAY` after the blur; a tray click whose
/// button-up arrives before that check finds the board still lifted and
/// drops it itself (the check then sees `PEEKING` cleared and stops).
const BLUR_GRACE: Duration = Duration::from_millis(400);
/// When the board was last lifted.
static LIFTED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Taking focus for a board that was in the background makes Windows report
/// a brief loss of focus first. A blur this soon after a lift is that
/// hand-over, not the user leaving, and must not end the peek.
const LIFT_GRACE: Duration = Duration::from_millis(600);
/// How long after a blur (and between re-checks) to look at who is in
/// front. Long enough for a drag or menu to take the foreground over.
const BLUR_CHECK_DELAY: Duration = Duration::from_millis(150);
/// A move or resize of the board this recent means the user is handling it.
const INTERACTION_GRACE: Duration = Duration::from_millis(800);
/// The board's own focus, as last reported by its window events. Read from
/// here rather than asking the window, which needs the event loop, and that
/// is busy inside a native move/size loop.
static BOARD_FOCUSED: AtomicBool = AtomicBool::new(false);
/// When the board last moved or resized.
static INTERACTED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Bumped by every blur and focus: only the newest blur check acts.
static BLUR_SEQ: AtomicU64 = AtomicU64::new(0);

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

/// What a pending blur check does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurCheck {
    /// The board has focus again: the peek goes on, stop checking.
    Keep,
    /// The user is still with this app (dragging, resizing, a menu or
    /// dialog of ours in front): look again after a while.
    Wait,
    /// Another app is in front: the user left, drop the board back.
    Drop,
}

/// Pure decision for one blur check while the board is lifted.
pub fn blur_check(
    since_lift: Option<Duration>,
    since_interaction: Option<Duration>,
    foreground_is_ours: bool,
    board_focused: bool,
) -> BlurCheck {
    if board_focused {
        BlurCheck::Keep
    } else if !blur_ends_peek(since_lift)
        || since_interaction.is_some_and(|d| d < INTERACTION_GRACE)
        || foreground_is_ours
    {
        BlurCheck::Wait
    } else {
        BlurCheck::Drop
    }
}

/// True when a blur check ends the peek now.
pub fn blur_drops(
    since_lift: Option<Duration>,
    since_interaction: Option<Duration>,
    foreground_is_ours: bool,
    board_focused: bool,
) -> bool {
    blur_check(
        since_lift,
        since_interaction,
        foreground_is_ours,
        board_focused,
    ) == BlurCheck::Drop
}

fn elapsed(at: &Mutex<Option<Instant>>) -> Option<Duration> {
    at.lock()
        .unwrap_or_else(|p| p.into_inner())
        .map(|at| at.elapsed())
}

fn stamp(at: &Mutex<Option<Instant>>) {
    *at.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
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
    if already_dropped(step, elapsed(&BLUR_DROPPED_AT)) {
        return Ok(());
    }
    match step {
        PeekStep::Show { lift: false } => {
            PREVIOUS.store(0, Ordering::SeqCst);
            windows::show_main(app)
        }
        PeekStep::Show { lift: true } | PeekStep::Lift => lift(app),
        PeekStep::Drop => {
            let focused = w.is_focused().unwrap_or(false);
            // Taken before `drop_back`, which clears it.
            let previous = PREVIOUS.swap(0, Ordering::SeqCst);
            drop_back(app, settings);
            // The board keeps the keyboard when it sinks; hand it back.
            if focused {
                foreground::restore(previous);
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
    // Always overwritten, so a window from an earlier peek is never the one
    // given the keyboard back.
    let previous = if w.is_focused().unwrap_or(false) {
        0
    } else {
        foreground::current()
    };
    PREVIOUS.store(previous, Ordering::SeqCst);
    PEEKING.store(true, Ordering::SeqCst);
    stamp(&LIFTED_AT);
    // Checks left over from an earlier peek belong to that peek.
    BLUR_SEQ.fetch_add(1, Ordering::SeqCst);
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

/// Show the board for a tray item or a second launch. A board on the
/// bottom layer is lifted, so it never owns the keyboard while hidden under
/// every other window.
pub fn reveal(app: &AppHandle) -> Result<(), String> {
    let settings = AppState::from_app(app).settings();
    if settings.always_on_bottom && !PEEKING.load(Ordering::SeqCst) {
        lift(app)
    } else {
        windows::show_main(app)
    }
}

/// Put a lifted board back on the layer from `settings`. No-op otherwise.
/// The remembered window is forgotten either way: the peek is over.
pub fn drop_back(app: &AppHandle, settings: &Settings) {
    PREVIOUS.store(0, Ordering::SeqCst);
    if !PEEKING.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window(windows::MAIN) {
        windows::apply_layer(&w, settings.always_on_top, settings.always_on_bottom);
    }
    log::info!("peek: dropped back");
}

/// The board gained focus: any pending blur check is moot.
pub fn on_focus() {
    BOARD_FOCUSED.store(true, Ordering::SeqCst);
    BLUR_SEQ.fetch_add(1, Ordering::SeqCst);
}

/// The board moved or was resized (by the user dragging it, or by us).
pub fn on_interaction() {
    stamp(&INTERACTED_AT);
}

/// The board (`board`) or the capture bar lost focus. That ends a peek only
/// if the user really went to another app, which is checked a moment later
/// and re-checked for as long as one of our own windows stays in front.
pub fn on_blur(app: &AppHandle, board: bool) {
    if board {
        BOARD_FOCUSED.store(false, Ordering::SeqCst);
    }
    if !PEEKING.load(Ordering::SeqCst) || !blur_ends_peek(elapsed(&LIFTED_AT)) {
        return;
    }
    // Overlapping blurs: each bump retires the checks scheduled before it.
    let seq = BLUR_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(BLUR_CHECK_DELAY).await;
            if BLUR_SEQ.load(Ordering::SeqCst) != seq || !PEEKING.load(Ordering::SeqCst) {
                return;
            }
            let step = blur_check(
                elapsed(&LIFTED_AT),
                elapsed(&INTERACTED_AT),
                foreground::is_ours(foreground::current()),
                BOARD_FOCUSED.load(Ordering::SeqCst),
            );
            match step {
                BlurCheck::Keep => return,
                BlurCheck::Wait => continue,
                BlurCheck::Drop => {
                    // Stamp before dropping, so a tray click racing this
                    // drop already sees it and does not lift again.
                    stamp(&BLUR_DROPPED_AT);
                    drop_back(&app, &AppState::from_app(&app).settings());
                    return;
                }
            }
        }
    });
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
    fn a_blur_while_another_app_is_in_front_drops_the_board() {
        let long_ago = Some(Duration::from_secs(5));
        assert!(blur_drops(long_ago, None, false, false));
        assert!(blur_drops(long_ago, long_ago, false, false));
        assert!(blur_drops(None, None, false, false));
    }

    #[test]
    fn dragging_or_resizing_the_board_keeps_the_peek() {
        let long_ago = Some(Duration::from_secs(5));
        let just_moved = Some(Duration::from_millis(100));
        // Even if the foreground momentarily reads as someone else's.
        assert!(!blur_drops(long_ago, just_moved, false, false));
        assert_eq!(
            blur_check(long_ago, just_moved, false, false),
            BlurCheck::Wait
        );
    }

    #[test]
    fn our_own_window_in_front_keeps_the_peek_and_keeps_watching() {
        let long_ago = Some(Duration::from_secs(5));
        assert_eq!(blur_check(long_ago, None, true, false), BlurCheck::Wait);
        assert!(!blur_drops(long_ago, None, true, false));
    }

    #[test]
    fn a_board_focused_again_ends_the_check() {
        let long_ago = Some(Duration::from_secs(5));
        assert_eq!(blur_check(long_ago, None, false, true), BlurCheck::Keep);
        assert_eq!(blur_check(long_ago, None, true, true), BlurCheck::Keep);
        assert!(!blur_drops(long_ago, None, false, true));
    }

    #[test]
    fn a_check_inside_the_lift_hand_over_waits() {
        let just_lifted = Some(Duration::from_millis(40));
        assert_eq!(blur_check(just_lifted, None, false, false), BlurCheck::Wait);
    }

    #[test]
    fn other_boards_toggle_like_the_tray() {
        assert_eq!(peek_step(true, false, false), PeekStep::Hide);
        assert_eq!(peek_step(true, false, true), PeekStep::Hide);
    }
}
