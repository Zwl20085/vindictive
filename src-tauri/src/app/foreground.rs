//! Which window has the keyboard, so a peek can give it back.
//!
//! Dropping the board to the bottom layer does not take focus away from it,
//! and keys typed next would land on a window the user can no longer see.
//! So the window in front is remembered when the board is lifted and
//! focused again when the board is dropped with the hotkey.

/// An opaque window handle; `0` means none.
pub type WindowId = isize;

#[cfg(windows)]
mod sys {
    #[link(name = "user32")]
    extern "system" {
        pub fn GetForegroundWindow() -> isize;
        pub fn SetForegroundWindow(hwnd: isize) -> i32;
    }
}

/// The window currently in front, or `0`.
#[cfg(windows)]
pub fn current() -> WindowId {
    // SAFETY: GetForegroundWindow takes no arguments, has no preconditions
    // and returns a handle value or null.
    unsafe { sys::GetForegroundWindow() }
}

/// Give the keyboard back to `window`. Returns false if Windows refused or
/// the window is gone; nothing else happens in that case.
#[cfg(windows)]
pub fn restore(window: WindowId) -> bool {
    if window == 0 {
        return false;
    }
    // SAFETY: SetForegroundWindow accepts any handle value; a stale or
    // invalid one makes it return 0 without touching memory.
    unsafe { sys::SetForegroundWindow(window) != 0 }
}

#[cfg(not(windows))]
pub fn current() -> WindowId {
    0
}

#[cfg(not(windows))]
pub fn restore(_window: WindowId) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_window_is_never_restored() {
        assert!(!restore(0));
    }
}
