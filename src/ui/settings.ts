import type { Dock, Language, Settings, Theme } from '../types';
import { isLanguage, LOCALES, t, tOr } from '../lib/i18n';
import { clampColumns, MAX_COLUMNS, MIN_COLUMNS } from './board';
import { el } from './dom';
import { DISCARD_CONFIRM_MS, readHighlights } from './edit-form';
import { errorMessage } from './errors';

export const MAX_WEATHER_LOCATION_CHARS = 80;
export const MIN_WINDOW_OPACITY = 20;
export const MAX_WINDOW_OPACITY = 100;
/** Matches the Rust default in settings.rs. */
export const DEFAULT_WINDOW_OPACITY = 50;

export interface SettingsActions {
  onSave: (settings: Settings) => Promise<void>;
  /** Folder picker; resolves to `null` when cancelled. */
  onPickFolder: () => Promise<string | null>;
  onOpenFolder: () => Promise<void>;
  onClose: () => void;
}

export interface SettingsOptions {
  settings: Settings;
  actions: SettingsActions;
}

const DOCKS: Dock[] = ['right', 'left', 'free'];
const THEMES: Theme[] = ['dark', 'light', 'nerv', 'cobalt', 'paper'];

function field(label: string, name: string, input: HTMLElement): HTMLElement {
  const id = `set-${name}`;
  input.setAttribute('id', id);
  return el('label', { className: 'set-field', for: id }, el('span', { className: 'set-label', text: label }), input);
}

function text(name: keyof Settings, value: string, type = 'text', extra: Record<string, string> = {}): HTMLInputElement {
  return el('input', { type, name, value, autocomplete: 'off', spellcheck: 'false', ...extra });
}

/** Select whose option labels are translated through the string table (raw value if untranslated). */
function select(name: keyof Settings, value: string, options: readonly string[]): HTMLSelectElement {
  const sel = el('select', { name });
  for (const o of options) sel.append(el('option', { value: o, selected: o === value, text: tOr(o) }));
  return sel;
}

function check(name: keyof Settings, value: boolean, label: string): HTMLElement {
  const input = el('input', { type: 'checkbox', name, checked: value });
  return el('label', { className: 'set-check' }, input, label);
}

/** Spellings of one modifier, folded to a single name. */
const MODIFIER_ALIASES = new Map<string, string>(Object.entries({
  ctrl: 'ctrl',
  control: 'ctrl',
  shift: 'shift',
  alt: 'alt',
  option: 'alt',
  super: 'super',
  meta: 'super',
  win: 'super',
  cmd: 'super',
  command: 'super',
  cmdorctrl: 'cmdorctrl',
  commandorcontrol: 'cmdorctrl',
}));

/**
 * Canonical form of a shortcut: lower case, modifiers deduplicated and sorted,
 * key last. `Ctrl+Shift+K`, ` shift + ctrl+k ` and `Control+Shift+K` agree.
 */
export function normaliseShortcut(shortcut: string): string {
  const tokens = shortcut
    .split('+')
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean);
  const modifiers = new Set<string>();
  const keys: string[] = [];
  for (const token of tokens) {
    const modifier = MODIFIER_ALIASES.get(token);
    if (modifier) modifiers.add(modifier);
    else keys.push(token);
  }
  return [...[...modifiers].sort(), ...keys].join('+');
}

/** Whether two shortcut strings name the same key combination. */
export function sameShortcut(a: string, b: string): boolean {
  return normaliseShortcut(a) === normaliseShortcut(b);
}

/** Same cap as a tip's own `highlight` list (`core/edit.rs`). */
export const MAX_HIGHLIGHT_WORDS = 32;

/**
 * Comma-separated highlight words (a word may contain spaces), trimmed and
 * deduplicated ignoring case, by the same rules as a tip's `highlight`.
 */
export function parseHighlightWords(text: string): string[] {
  const words = readHighlights(text);
  if (words.length > MAX_HIGHLIGHT_WORDS) throw new Error(`${t('highlightWords')}: ≤ ${MAX_HIGHLIGHT_WORDS}`);
  return words;
}

/** Read the form back into a new Settings object, validated. */
export function readSettings(form: HTMLFormElement, base: Settings): Settings {
  const data = new FormData(form);
  const str = (k: keyof Settings): string => String(data.get(k) ?? '').trim();
  const bool = (k: keyof Settings): boolean => data.get(k) !== null;
  const dock = str('dock') as Dock;
  const theme = str('theme') as Theme;
  const language = str('language');
  const folder = str('folder');
  if (!folder) throw new Error(t('folderRequired'));
  if (!DOCKS.includes(dock)) throw new Error(`${t('invalidDock')}: ${dock}`);
  if (!THEMES.includes(theme)) throw new Error(`${t('invalidTheme')}: ${theme}`);
  if (!isLanguage(language)) throw new Error(`${t('invalidLanguage')}: ${language}`);
  const hotkey = str('hotkey');
  if (!hotkey) throw new Error(t('hotkeyRequired'));
  const boardHotkey = str('board_hotkey');
  if (boardHotkey && sameShortcut(boardHotkey, hotkey)) throw new Error(t('hotkeysClash'));
  const alwaysOnTop = bool('always_on_top');
  const alwaysOnBottom = bool('always_on_bottom');
  if (alwaysOnTop && alwaysOnBottom) throw new Error(t('layerConflict'));
  return {
    ...base,
    folder,
    hotkey,
    board_hotkey: boardHotkey,
    auto_update_check: bool('auto_update_check'),
    dock,
    theme,
    language: language as Language,
    columns: clampColumns(Number(str('columns'))),
    weather_location: str('weather_location').slice(0, MAX_WEATHER_LOCATION_CHARS),
    editor_command: str('editor_command'),
    fit_height: bool('fit_height'),
    window_opacity: Math.min(MAX_WINDOW_OPACITY, Math.max(MIN_WINDOW_OPACITY, Math.round(Number(str('window_opacity')) || DEFAULT_WINDOW_OPACITY))),
    show_panel: bool('show_panel'),
    always_on_top: alwaysOnTop,
    always_on_bottom: alwaysOnBottom,
    autostart: bool('autostart'),
    notify_new_tips: bool('notify_new_tips'),
    show_done: bool('show_done'),
    highlight_words: parseHighlightWords(String(data.get('highlight_words') ?? '')),
  };
}

/** The tips folder: a path field plus Browse and Open buttons. */
function folderSection(value: string, actions: SettingsActions, status: HTMLElement): HTMLElement {
  const input = text('folder', value, 'text', { placeholder: 'C:\\Users\\you\\OneDrive\\Vindictive' });
  const browse = el('button', { type: 'button', className: 'action', text: `${t('browse')}…` });
  const open = el('button', { type: 'button', className: 'action', text: t('openFolder') });
  const report = (error: unknown): void => {
    status.textContent = errorMessage(error);
    status.dataset.state = 'error';
  };
  browse.addEventListener('click', () => {
    actions
      .onPickFolder()
      .then((picked) => {
        if (picked) input.value = picked;
      })
      .catch(report);
  });
  open.addEventListener('click', () => void actions.onOpenFolder().catch(report));
  return el(
    'div',
    { className: 'set-folder' },
    el('p', { className: 'set-hint', text: t('folderHint') }),
    field(t('tipsFolder'), 'folder', input),
    el('div', { className: 'set-row' }, browse, open),
  );
}

/** Ticking either checkbox clears the other, so the pair can never both be on. */
function exclusiveChecks(form: HTMLFormElement, a: keyof Settings, b: keyof Settings): void {
  const box = (name: string): HTMLInputElement | null => form.querySelector<HTMLInputElement>(`input[name="${name}"]`);
  const first = box(a);
  const second = box(b);
  if (!first || !second) return;
  const link = (self: HTMLInputElement, other: HTMLInputElement): void =>
    self.addEventListener('change', () => {
      if (self.checked) other.checked = false;
    });
  link(first, second);
  link(second, first);
}

function group(title: string, ...children: (HTMLElement | null)[]): HTMLElement {
  return el('fieldset', { className: 'set-group' }, el('legend', { className: 'set-group-title', text: title }), ...children);
}

/** Settings overlay for the main window. */
export function renderSettings(options: SettingsOptions): HTMLElement {
  const { settings: s, actions } = options;
  const status = el('p', { className: 'set-status', role: 'status' });
  const form = el(
    'form',
    { className: 'settings', 'aria-label': t('settings') },
    el('h1', { className: 'set-title', text: t('settings') }),
    group(t('tipsFolder'), folderSection(s.folder, actions, status)),
    group(
      t('theme'),
      field(t('language'), 'language', select('language', s.language, LOCALES)),
      field(t('theme'), 'theme', select('theme', s.theme, THEMES)),
      field(`${t('columns')} (${MIN_COLUMNS}-${MAX_COLUMNS})`, 'columns', text('columns', String(s.columns), 'number')),
      field(t('weatherLocation'), 'weather_location', text('weather_location', s.weather_location, 'text', { placeholder: 'Tokyo', maxlength: String(MAX_WEATHER_LOCATION_CHARS) })),
      field(t('windowOpacity'), 'window_opacity', text('window_opacity', String(s.window_opacity), 'number', { min: String(MIN_WINDOW_OPACITY), max: String(MAX_WINDOW_OPACITY), step: '5' })),
      field(t('highlightWords'), 'highlight_words', text('highlight_words', (s.highlight_words ?? []).join(', '), 'text', { placeholder: 'Reviewer 2, ORCID', 'aria-describedby': 'set-highlight-hint' })),
      el('p', { className: 'set-hint', id: 'set-highlight-hint', text: t('highlightWordsHint') }),
      check('show_panel', s.show_panel, t('showPanel')),
      check('fit_height', s.fit_height, t('fitHeight')),
      check('show_done', s.show_done, t('showDoneTiles')),
    ),
    group(
      t('dock'),
      field(t('hotkey'), 'hotkey', text('hotkey', s.hotkey)),
      field(t('boardHotkey'), 'board_hotkey', text('board_hotkey', s.board_hotkey, 'text', { placeholder: 'Ctrl+Alt+Shift+Space' })),
      field(t('editorCommand'), 'editor_command', text('editor_command', s.editor_command, 'text', { placeholder: 'code' })),
      field(t('dock'), 'dock', select('dock', s.dock, DOCKS)),
      check('always_on_top', s.always_on_top, t('alwaysOnTop')),
      check('always_on_bottom', s.always_on_bottom, t('alwaysOnBottom')),
      check('autostart', s.autostart, t('autostart')),
      check('notify_new_tips', s.notify_new_tips, t('notifyNew')),
      check('auto_update_check', s.auto_update_check, t('autoUpdateCheck')),
    ),
    status,
    el(
      'div',
      { className: 'set-row set-actions' },
      el('button', { type: 'submit', className: 'action action-primary', text: t('save') }),
      el('button', { type: 'button', className: 'action action-back', text: t('back'), 'data-role': 'back' }),
    ),
  );
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    void (async () => {
      try {
        await actions.onSave(readSettings(form, s));
        status.textContent = t('saved');
        status.dataset.state = 'ok';
      } catch (error) {
        status.textContent = errorMessage(error);
        status.dataset.state = 'error';
      }
    })();
  });
  form.querySelector<HTMLButtonElement>('[data-role="back"]')?.addEventListener('click', actions.onClose);
  exclusiveChecks(form, 'always_on_top', 'always_on_bottom');
  return form;
}

/** The form's values in a comparable form. */
function snapshot(form: HTMLFormElement): string {
  return JSON.stringify(Array.from(new FormData(form).entries(), ([k, v]) => [k, String(v)]));
}

const NO_ACTIONS: SettingsActions = {
  onSave: async () => undefined,
  onPickFolder: async () => null,
  onOpenFolder: async () => undefined,
  onClose: () => undefined,
};

/**
 * The form differs from `settings`: its current values are compared with
 * those of a fresh form for `settings`, so both sides go through the same
 * rendering and nothing reads as changed merely by normalisation.
 */
export function isSettingsDirty(form: HTMLFormElement, settings: Settings): boolean {
  return snapshot(form) !== snapshot(renderSettings({ settings, actions: NO_ACTIONS }) as HTMLFormElement);
}

/** Per form: until when a second Esc discards, and the timer that disarms. */
const armed = new WeakMap<HTMLFormElement, { until: number; timer: ReturnType<typeof setTimeout> }>();

/**
 * Esc in Settings, like the edit form: true (leave) at once when nothing
 * changed, or on a second Esc within `DISCARD_CONFIRM_MS`; otherwise false,
 * with a hint in the status line.
 */
export function settingsEscape(form: HTMLFormElement, settings: Settings): boolean {
  const status = form.querySelector<HTMLElement>('.set-status');
  const disarm = (): void => {
    const arm = armed.get(form);
    if (arm) clearTimeout(arm.timer);
    armed.delete(form);
    if (status?.dataset.state === 'confirm') {
      status.textContent = '';
      status.dataset.state = '';
    }
  };
  const arm = armed.get(form);
  if (!isSettingsDirty(form, settings) || (arm && Date.now() < arm.until)) {
    disarm();
    return true;
  }
  if (arm) clearTimeout(arm.timer);
  armed.set(form, { until: Date.now() + DISCARD_CONFIRM_MS, timer: setTimeout(disarm, DISCARD_CONFIRM_MS) });
  if (status) {
    status.textContent = t('discardHint');
    status.dataset.state = 'confirm';
  }
  return false;
}
