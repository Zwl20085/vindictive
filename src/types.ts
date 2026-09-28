/**
 * IPC contract between the Tauri (Rust) backend and the web frontend.
 * Mirrors `src-tauri/src/core/tip.rs` and `src-tauri/src/app/state.rs`.
 *
 * All timestamps ending in `_at` are chrono `NaiveDateTime` values serialised
 * as `YYYY-MM-DDTHH:MM:SS` in *local* time with no zone suffix. Treat them as
 * wall-clock time; never call `Date.parse` with a `Z`.
 */

export type Kind = 'task' | 'deadline' | 'note' | 'reading' | 'event';
export type Priority = 'low' | 'normal' | 'high';
export type Status = 'open' | 'done';
export type Urgency = 'none' | 'later' | 'soon' | 'critical' | 'overdue';
export type Dock = 'left' | 'right' | 'free';
export type Theme = 'dark' | 'light' | 'nerv' | 'cobalt' | 'paper';
export type Language = 'en' | 'zh';

export interface Paper {
  title: string;
  authors: string[];
  year?: number;
  venue?: string;
  url?: string;
}

export interface Tip {
  /** File stem, stable id. */
  id: string;
  /** Path inside the tips repository. */
  path: string;
  sha?: string;

  title: string;
  kind: Kind;
  priority: Priority;
  status: Status;
  /** Raw frontmatter strings, kept for display/edit only. */
  due?: string;
  remind?: string[];
  location?: string;
  links?: string[];
  /** Figures (png, jpg, svg, …) relative to the tips directory or absolute URLs. */
  images?: string[];
  tags?: string[];
  repeat?: string;
  /** Optional CSS colour override for the tile. */
  color?: string;
  arxiv?: string;
  doi?: string;
  paper?: Paper;
  snoozed_until?: string;
  done_at?: string;
  created?: string;

  /** Markdown body. */
  body: string;

  /** Resolved local timestamps. */
  due_at?: string;
  remind_at: string[];
  snoozed_until_at?: string;
}

export interface Settings {
  owner: string;
  repo: string;
  branch: string;
  /** Directory inside the repo holding tip files, e.g. `tips`. */
  dir: string;
  poll_seconds: number;
  /** Global shortcut for quick capture, e.g. `Ctrl+Shift+Space`. */
  hotkey: string;
  dock: Dock;
  always_on_top: boolean;
  autostart: boolean;
  notify_new_tips: boolean;
  theme: Theme;
  /** Grid columns on the board (2..6). */
  columns: number;
  show_done: boolean;
  /** UI language. */
  language: Language;
  /** City or place name for the weather panel; empty disables weather. */
  weather_location: string;
  /** Show the clock / date / weather panel above the board. */
  show_panel: boolean;
  /** Command for "Edit locally" (e.g. `code`); blank = system default app. */
  editor_command: string;
}

/** Current conditions, fetched by the backend from Open-Meteo. */
export interface Weather {
  /** Resolved place name, e.g. `Tokyo`. */
  location: string;
  temperature_c: number;
  high_c?: number;
  low_c?: number;
  humidity?: number;
  wind_kmh?: number;
  /** WMO weather interpretation code (0..99). */
  code: number;
  is_day: boolean;
  /** Naive local timestamp of the fetch. */
  fetched_at: string;
}

export interface BoardState {
  /** Every tip including done ones, already ordered by the backend
   *  (highest score first). The frontend hides done tips unless asked. */
  tips: Tip[];
  /** Id of the tip to highlight as "next up". */
  next_up?: string;
  last_sync?: string;
  sync_error?: string;
  has_token: boolean;
  settings: Settings;
  /** Backend local time at the moment the state was produced. */
  now: string;
}

/**
 * Tauri command names and signatures. Every mutating command returns the new
 * `BoardState`; the backend also emits `board-updated` with the same payload
 * so all windows stay in sync.
 */
export interface Commands {
  get_state: () => BoardState;
  sync_now: () => BoardState;
  mark_done: (args: { id: string }) => BoardState;
  reopen: (args: { id: string }) => BoardState;
  snooze: (args: { id: string; minutes: number }) => BoardState;
  /** Remove the tip's file from the repository. Irreversible except through git history. */
  delete_tip: (args: { id: string }) => BoardState;
  /** Write the tip to the local edit folder, open it in the editor, and push every save. Returns the path. */
  edit_local: (args: { id: string }) => string;
  /** Quick-capture line, see `core/capture.rs` for the syntax. */
  create_tip: (args: { text: string }) => BoardState;
  save_settings: (args: { settings: Settings }) => BoardState;
  set_token: (args: { token: string }) => void;
  clear_token: () => void;
  /** Human readable result, e.g. `OK: 12 tips in owner/repo/tips`. Throws on failure. */
  test_connection: () => string;
  /** Resolve an image reference (relative to the tips dir, repo path, or http URL)
   *  to something an `<img>` can display: a data URL or the URL itself. */
  fetch_image: (args: { path: string }) => string;
  /** Fetch arXiv / Crossref metadata into `paper`. */
  enrich_tip: (args: { id: string }) => BoardState;
  /** Current weather for `settings.weather_location`; `null` when unset. Cached ~20 min. */
  fetch_weather: () => Weather | null;
  show_capture: () => void;
  hide_capture: () => void;
  /** Move the board to a screen edge or leave it free. */
  dock_window: (args: { dock: Dock }) => void;
  /** Append a line from the UI to the app log file. */
  frontend_log: (args: { level: 'error' | 'warn' | 'info'; message: string }) => void;
  quit: () => void;
}

/** Events emitted by the backend. */
export interface Events {
  /** Fired after every state change. */
  'board-updated': BoardState;
  /** Fired when the capture window is shown; focus the input. */
  'capture-shown': null;
  /** Fired by the tray menu; the main window should open the settings overlay. */
  'open-settings': null;
}

export const WINDOW_MAIN = 'main';
export const WINDOW_CAPTURE = 'capture';
