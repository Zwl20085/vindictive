import type { Dock, Language, Settings, Theme } from '../types';
import { isLanguage, LOCALES, t, type StringKey } from '../lib/i18n';
import { clampColumns, MAX_COLUMNS, MIN_COLUMNS } from './board';
import { el } from './dom';

export const MIN_POLL_SECONDS = 15;
export const MAX_WEATHER_LOCATION_CHARS = 80;

export interface SettingsActions {
  onSave: (settings: Settings) => Promise<void>;
  onSetToken: (token: string) => Promise<void>;
  onClearToken: () => Promise<void>;
  onTest: () => Promise<string>;
  onClose: () => void;
}

export interface SettingsOptions {
  settings: Settings;
  hasToken: boolean;
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

/** Select whose option labels are translated through the string table. */
function select(name: keyof Settings, value: string, options: readonly string[]): HTMLSelectElement {
  const sel = el('select', { name });
  for (const o of options) sel.append(el('option', { value: o, selected: o === value, text: t(o as StringKey) }));
  return sel;
}

function check(name: keyof Settings, value: boolean, label: string): HTMLElement {
  const input = el('input', { type: 'checkbox', name, checked: value });
  return el('label', { className: 'set-check' }, input, label);
}

/** Read the form back into a new Settings object, validated. */
export function readSettings(form: HTMLFormElement, base: Settings): Settings {
  const data = new FormData(form);
  const str = (k: keyof Settings): string => String(data.get(k) ?? '').trim();
  const bool = (k: keyof Settings): boolean => data.get(k) !== null;
  const poll = Number(str('poll_seconds'));
  const dock = str('dock') as Dock;
  const theme = str('theme') as Theme;
  const language = str('language');
  if (!str('owner') || !str('repo')) throw new Error('owner and repo are required');
  if (!Number.isFinite(poll) || poll < MIN_POLL_SECONDS) throw new Error(`poll interval must be at least ${MIN_POLL_SECONDS} s`);
  if (!DOCKS.includes(dock)) throw new Error('invalid dock');
  if (!THEMES.includes(theme)) throw new Error('invalid theme');
  if (!isLanguage(language)) throw new Error('invalid language');
  return {
    ...base,
    owner: str('owner'),
    repo: str('repo'),
    branch: str('branch') || 'main',
    dir: str('dir').replace(/^\/+|\/+$/g, ''),
    poll_seconds: Math.round(poll),
    hotkey: str('hotkey') || base.hotkey,
    dock,
    theme,
    language: language as Language,
    columns: clampColumns(Number(str('columns'))),
    weather_location: str('weather_location').slice(0, MAX_WEATHER_LOCATION_CHARS),
    editor_command: str('editor_command'),
    push_interval_minutes: Math.max(0, Math.round(Number(str('push_interval_minutes')) || 0)),
    fit_height: bool('fit_height'),
    show_panel: bool('show_panel'),
    always_on_top: bool('always_on_top'),
    autostart: bool('autostart'),
    notify_new_tips: bool('notify_new_tips'),
    show_done: bool('show_done'),
  };
}

function tokenSection(hasToken: boolean, actions: SettingsActions, status: HTMLElement): HTMLElement {
  const input = el('input', { type: 'password', name: 'token', placeholder: hasToken ? t('tokenStored') : 'github_pat_…', autocomplete: 'off' });
  const save = el('button', { type: 'button', className: 'action', text: t('saveToken') });
  const clearBtn = el('button', { type: 'button', className: 'action', text: t('clear') });
  const test = el('button', { type: 'button', className: 'action', text: t('testConnection') });
  const run = async (label: string, fn: () => Promise<string | void>): Promise<void> => {
    status.textContent = `${label}…`;
    status.dataset.state = 'busy';
    try {
      const result = await fn();
      status.textContent = typeof result === 'string' ? result : `${label} ${t('doneSuffix')}`;
      status.dataset.state = 'ok';
    } catch (error) {
      status.textContent = error instanceof Error ? error.message : String(error);
      status.dataset.state = 'error';
    }
  };
  save.addEventListener('click', () => {
    const value = input.value.trim();
    if (!value) return void run(t('saveToken'), async () => Promise.reject(new Error(t('tokenEmpty'))));
    void run(t('saveToken'), async () => {
      await actions.onSetToken(value);
      input.value = '';
      input.placeholder = t('tokenStored');
    });
  });
  clearBtn.addEventListener('click', () => void run(t('clearToken'), () => actions.onClearToken()));
  test.addEventListener('click', () => void run(t('testing'), () => actions.onTest()));
  return el('section', { className: 'set-token' }, field(t('githubToken'), 'token', input), el('div', { className: 'set-row' }, save, clearBtn, test));
}

function group(title: string, ...children: (HTMLElement | null)[]): HTMLElement {
  return el('fieldset', { className: 'set-group' }, el('legend', { className: 'set-group-title', text: title }), ...children);
}

/** Settings overlay for the main window. */
export function renderSettings(options: SettingsOptions): HTMLElement {
  const { settings: s, hasToken, actions } = options;
  const status = el('p', { className: 'set-status', role: 'status' });
  const form = el(
    'form',
    { className: 'settings', 'aria-label': t('settings') },
    el('h1', { className: 'set-title', text: t('settings') }),
    group(
      'GitHub',
      field(t('owner'), 'owner', text('owner', s.owner)),
      field(t('repo'), 'repo', text('repo', s.repo)),
      field(t('branch'), 'branch', text('branch', s.branch)),
      field(t('directory'), 'dir', text('dir', s.dir)),
      tokenSection(hasToken, actions, status),
      field(t('pollSeconds'), 'poll_seconds', text('poll_seconds', String(s.poll_seconds), 'number')),
    ),
    group(
      t('theme'),
      field(t('language'), 'language', select('language', s.language, LOCALES)),
      field(t('theme'), 'theme', select('theme', s.theme, THEMES)),
      field(`${t('columns')} (${MIN_COLUMNS}-${MAX_COLUMNS})`, 'columns', text('columns', String(s.columns), 'number')),
      field(t('weatherLocation'), 'weather_location', text('weather_location', s.weather_location, 'text', { placeholder: 'Tokyo', maxlength: String(MAX_WEATHER_LOCATION_CHARS) })),
      check('show_panel', s.show_panel, t('showPanel')),
      check('fit_height', s.fit_height, t('fitHeight')),
      check('show_done', s.show_done, t('showDoneTiles')),
    ),
    group(
      t('dock'),
      field(t('hotkey'), 'hotkey', text('hotkey', s.hotkey)),
      field(t('editorCommand'), 'editor_command', text('editor_command', s.editor_command, 'text', { placeholder: 'code' })),
      field(t('pushInterval'), 'push_interval_minutes', text('push_interval_minutes', String(s.push_interval_minutes), 'number', { min: '0' })),
      field(t('dock'), 'dock', select('dock', s.dock, DOCKS)),
      check('always_on_top', s.always_on_top, t('alwaysOnTop')),
      check('autostart', s.autostart, t('autostart')),
      check('notify_new_tips', s.notify_new_tips, t('notifyNew')),
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
        status.textContent = error instanceof Error ? error.message : String(error);
        status.dataset.state = 'error';
      }
    })();
  });
  form.querySelector<HTMLButtonElement>('[data-role="back"]')?.addEventListener('click', actions.onClose);
  return form;
}
