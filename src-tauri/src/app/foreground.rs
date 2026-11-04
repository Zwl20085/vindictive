//! Which window has the keyboard, so a peek can give it back.
//!
//! Dropping the board to the bottom layer does not take focus away from it,
//! and keys typed next would land on a window the user can no longer see.
//! So the window in front is remembered when the board is lifted and
//! focused again when the board is dropped with the hotkey.

/// An opaque window handle; `0` means none.
pub type WindowId = isize;

/// How many parent links to follow from a window's process to ours. WebView2
/// runs `msedgewebview2.exe` as our child (browser) and grandchildren
/// (renderer, GPU, utility); a few levels cover them with room to spare.
pub const MAX_ANCESTRY_DEPTH: usize = 4;

#[cfg(windows)]
mod sys {
    #[link(name = "user32")]
    extern "system" {
        pub fn GetForegroundWindow() -> isize;
        pub fn SetForegroundWindow(hwnd: isize) -> i32;
        pub fn GetWindowThreadProcessId(hwnd: isize, process_id: *mut u32) -> u32;
        pub fn GetAncestor(hwnd: isize, flags: u32) -> isize;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> isize;
        pub fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        pub fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        pub fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> isize;
        pub fn GetProcessTimes(
            process: isize,
            creation: *mut u64,
            exit: *mut u64,
            kernel: *mut u64,
            user: *mut u64,
        ) -> i32;
        pub fn CloseHandle(handle: isize) -> i32;
    }

    /// `GetAncestor` flag: the root window's owner chain, to its end.
    pub const GA_ROOTOWNER: u32 = 3;
    /// `CreateToolhelp32Snapshot` flag: every process in the system.
    pub const TH32CS_SNAPPROCESS: u32 = 0x2;
    pub const INVALID_HANDLE_VALUE: isize = -1;
    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    /// `PROCESSENTRY32W`, laid out as in `tlhelp32.h`.
    #[repr(C)]
    pub struct ProcessEntry32W {
        pub size: u32,
        pub usage: u32,
        pub process_id: u32,
        pub default_heap_id: usize,
        pub module_id: u32,
        pub threads: u32,
        pub parent_process_id: u32,
        pub priority_class_base: i32,
        pub flags: u32,
        pub exe_file: [u16; 260],
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

/// The process that owns `window`, or `0` if it is gone.
#[cfg(windows)]
fn process_of(window: WindowId) -> u32 {
    if window == 0 {
        return 0;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid, writable u32 for the duration of the call;
    // an invalid handle makes the call return 0 and leave `pid` at 0.
    unsafe { sys::GetWindowThreadProcessId(window, &mut pid) };
    pid
}

/// True when `window` belongs to this app: one of our own windows (board,
/// capture bar, native dialogs we open), a popup owned by one of them, or a
/// window of a process we started (WebView2's context menus and pickers
/// live in `msedgewebview2.exe`). Asks Win32 directly, never the Tauri event
/// loop, so it is safe to call from a background task while the board is
/// in a native move/size loop.
#[cfg(windows)]
pub fn is_ours(window: WindowId) -> bool {
    if window == 0 {
        return false;
    }
    // SAFETY: GetAncestor accepts any handle value and returns null for an
    // invalid one.
    let root_owner = unsafe { sys::GetAncestor(window, sys::GA_ROOTOWNER) };
    let processes = [process_of(window), process_of(root_owner)];
    let ours = std::process::id();
    owned_by_us(processes, ours)
        || processes
            .iter()
            .any(|&pid| pid != 0 && lineage::descends_from_us(pid, ours))
}

/// Process ancestry, cached: the blur check asks every 150 ms and a process
/// snapshot is far too heavy for that.
#[cfg(windows)]
mod lineage {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::sys;

    /// How long one answer for a process id stays good.
    const CACHE_TTL: Duration = Duration::from_secs(5);
    static CACHE: Mutex<Vec<(u32, bool, Instant)>> = Mutex::new(Vec::new());

    pub fn descends_from_us(pid: u32, ours: u32) -> bool {
        let mut cache = CACHE.lock().unwrap_or_else(|p| p.into_inner());
        cache.retain(|(_, _, at)| at.elapsed() < CACHE_TTL);
        if let Some(&(_, answer, _)) = cache.iter().find(|(p, _, _)| *p == pid) {
            return answer;
        }
        let parents = parent_map();
        let answer = super::descends_from(
            pid,
            ours,
            |child| {
                let parent = *parents.get(&child)?;
                // Process ids are reused: a "parent" started after its child
                // is an unrelated process that took a dead parent's id.
                match (created_at(parent), created_at(child)) {
                    (Some(p), Some(c)) if p <= c => Some(parent),
                    _ => None,
                }
            },
            super::MAX_ANCESTRY_DEPTH,
        );
        cache.push((pid, answer, Instant::now()));
        answer
    }

    /// Every running process's parent id, from one toolhelp snapshot.
    fn parent_map() -> HashMap<u32, u32> {
        let mut map = HashMap::new();
        // SAFETY: plain call; failure returns INVALID_HANDLE_VALUE, checked.
        let snapshot = unsafe { sys::CreateToolhelp32Snapshot(sys::TH32CS_SNAPPROCESS, 0) };
        if snapshot == sys::INVALID_HANDLE_VALUE {
            return map;
        }
        // SAFETY: an all-zero PROCESSENTRY32W is valid plain data.
        let mut entry: sys::ProcessEntry32W = unsafe { std::mem::zeroed() };
        entry.size = std::mem::size_of::<sys::ProcessEntry32W>() as u32;
        // SAFETY: `snapshot` is a live snapshot handle and `entry` a writable
        // PROCESSENTRY32W whose `size` is set, as both calls require.
        let mut more = unsafe { sys::Process32FirstW(snapshot, &mut entry) } != 0;
        while more {
            map.insert(entry.process_id, entry.parent_process_id);
            // SAFETY: as above.
            more = unsafe { sys::Process32NextW(snapshot, &mut entry) } != 0;
        }
        // SAFETY: `snapshot` is a handle we own and close once.
        unsafe { sys::CloseHandle(snapshot) };
        map
    }

    /// When process `pid` started (FILETIME ticks), or `None` if it is gone
    /// or not ours to query.
    fn created_at(pid: u32) -> Option<u64> {
        // SAFETY: plain call; failure returns null, checked.
        let handle = unsafe { sys::OpenProcess(sys::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle == 0 {
            return None;
        }
        let (mut creation, mut exit, mut kernel, mut user) = (0u64, 0u64, 0u64, 0u64);
        // SAFETY: `handle` is a live process handle with query rights; the
        // four out-pointers are valid, writable 8-byte FILETIMEs.
        let ok = unsafe {
            sys::GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
        } != 0;
        // SAFETY: `handle` is a handle we own and close once.
        unsafe { sys::CloseHandle(handle) };
        ok.then_some(creation)
    }
}

#[cfg(not(windows))]
pub fn current() -> WindowId {
    0
}

#[cfg(not(windows))]
pub fn restore(_window: WindowId) -> bool {
    false
}

#[cfg(not(windows))]
pub fn is_ours(_window: WindowId) -> bool {
    false
}

/// Pure ownership rule: any of the window's processes (itself, its root
/// owner) is ours. `0` means unknown and never matches.
pub fn owned_by_us(processes: [u32; 2], ours: u32) -> bool {
    ours != 0 && processes.contains(&ours)
}

/// Pure ancestry rule: following `parent_of` from `pid` reaches `ours`
/// within `max_depth` links. `parent_of` returns `None` for an unknown or
/// untrusted link (a reused id), which ends the walk.
pub fn descends_from(
    pid: u32,
    ours: u32,
    parent_of: impl Fn(u32) -> Option<u32>,
    max_depth: usize,
) -> bool {
    if ours == 0 {
        return false;
    }
    let mut at = pid;
    for _ in 0..=max_depth {
        if at == ours {
            return true;
        }
        match parent_of(at) {
            Some(parent) if parent != 0 && parent != at => at = parent,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn no_window_is_never_restored() {
        assert!(!restore(0));
    }

    #[test]
    fn no_window_is_never_ours() {
        assert!(!is_ours(0));
    }

    #[test]
    fn ownership_follows_the_window_or_its_owner() {
        assert!(owned_by_us([42, 0], 42));
        assert!(owned_by_us([7, 42], 42));
        assert!(!owned_by_us([7, 9], 42));
        assert!(!owned_by_us([0, 0], 0));
    }

    fn walk(map: &HashMap<u32, u32>, pid: u32, ours: u32) -> bool {
        descends_from(pid, ours, |c| map.get(&c).copied(), MAX_ANCESTRY_DEPTH)
    }

    #[test]
    fn webview_children_and_grandchildren_are_ours() {
        // explorer(4) -> us(10) -> webview browser(11) -> renderer(12)
        let map = HashMap::from([(10, 4), (11, 10), (12, 11), (20, 4)]);
        assert!(walk(&map, 10, 10));
        assert!(walk(&map, 11, 10));
        assert!(walk(&map, 12, 10));
        // A sibling under explorer is not ours.
        assert!(!walk(&map, 20, 10));
        // Unknown processes and id 0 are not ours.
        assert!(!walk(&map, 99, 10));
        assert!(!walk(&map, 12, 0));
    }

    #[test]
    fn the_walk_is_bounded_and_survives_cycles() {
        // A chain deeper than the limit stops before reaching us.
        let deep: HashMap<u32, u32> = (1..=10).map(|p| (p + 1, p)).collect();
        assert!(walk(&deep, 3, 1));
        assert!(!walk(&deep, 11, 1));
        // A cycle (reused ids) never loops forever.
        let cycle = HashMap::from([(5, 6), (6, 5)]);
        assert!(!walk(&cycle, 5, 10));
        // A self-parented entry (the idle process) ends the walk.
        let idle = HashMap::from([(0, 0), (7, 7)]);
        assert!(!walk(&idle, 7, 10));
    }

    #[test]
    fn an_untrusted_link_breaks_the_chain() {
        // `parent_of` refusing a link (say, a parent younger than its child)
        // means the walk stops there.
        let refuses_11 = |c: u32| (c == 12).then_some(11);
        assert!(!descends_from(12, 10, refuses_11, MAX_ANCESTRY_DEPTH));
        let trusts_all = |c: u32| {
            [(12, 11), (11, 10)]
                .into_iter()
                .find(|&(k, _)| k == c)
                .map(|(_, p)| p)
        };
        assert!(descends_from(12, 10, trusts_all, MAX_ANCESTRY_DEPTH));
    }
}
