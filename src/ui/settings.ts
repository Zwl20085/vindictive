import type { Dock, Settings, Theme } from '../types';
import { clampColumns, MAX_COLUMNS, MIN_COLUMNS } from './board';
import { el } from './dom';

export const MIN_POLL_SECONDS = 15;

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
const THEMES: Theme[] = ['dark', 'light'];

function field(label: string, input: HTMLElement): HTMLElement {
  const id = `set-${label.toLowerCase().replace(/\W+/g, '-')}`;
  input.setAttribute('id', id);
  return el('label', { className: 'set-field', for: id }, el('span', { className: 'set-label', text: label }), input);
}

function text(name: keyof Settings, value: string, type = 'text'): HTMLInputElement {
  return el('input', { type, name, value, autocomplete: 'off', spellcheck: 'false' });
}

function select(name: keyof Settings, value: string, options: readonly string[]): HTMLSelectElement {
  const sel = el('select', { name });
  for (const o of options) sel.append(el('option', { value: o, selected: o === value, text: o }));
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
  if (!str('owner') || !str('repo')) throw new Error('owner and repo are required');
  if (!Number.isFinite(poll) || poll < MIN_POLL_SECONDS) throw new Error(`poll interval must be at least ${MIN_POLL_SECONDS} s`);
  if (!DOCKS.includes(dock)) throw new Error('invalid dock');
  if (!THEMES.includes(theme)) throw new Error('invalid theme');
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
    columns: clampColumns(Number(str('columns'))),
    always_on_top: bool('always_on_top'),
    autostart: bool('autostart'),
    notify_new_tips: bool('notify_new_tips'),
    show_done: bool('show_done'),
  };
}

function tokenSection(hasToken: boolean, actions: SettingsActions, status: HTMLElement): HTMLElement {
  const input = el('input', { type: 'password', name: 'token', placeholder: hasToken ? 'token stored' : 'github_pat_…', autocomplete: 'off' });
  const save = el('button', { type: 'button', className: 'action', text: 'Save token' });
  const clearBtn = el('button', { type: 'button', className: 'action', text: 'Clear' });
  const test = el('button', { type: 'button', className: 'action', text: 'Test connection' });
  const run = async (label: string, fn: () => Promise<string | void>): Promise<void> => {
    status.textContent = `${label}…`;
    status.dataset.state = 'busy';
    try {
      const result = await fn();
      status.textContent = typeof result === 'string' ? result : `${label} done`;
      status.dataset.state = 'ok';
    } catch (error) {
      status.textContent = error instanceof Error ? error.message : String(error);
      status.dataset.state = 'error';
    }
  };
  save.addEventListener('click', () => {
    const value = input.value.trim();
    if (!value) return void run('Save token', async () => Promise.reject(new Error('token is empty')));
    void run('Save token', async () => {
      await actions.onSetToken(value);
      input.value = '';
      input.placeholder = 'token stored';
    });
  });
  clearBtn.addEventListener('click', () => void run('Clear token', () => actions.onClearToken()));
  test.addEventListener('click', () => void run('Testing', () => actions.onTest()));
  return el('section', { className: 'set-token' }, field('GitHub token', input), el('div', { className: 'set-row' }, save, clearBtn, test));
}

/** Settings overlay for the main window. */
export function renderSettings(options: SettingsOptions): HTMLElement {
  const { settings: s, hasToken, actions } = options;
  const status = el('p', { className: 'set-status', role: 'status' });
  const form = el(
    'form',
    { className: 'settings', 'aria-label': 'Settings' },
    el('h1', { className: 'set-title', text: 'Settings' }),
    field('Owner', text('owner', s.owner)),
    field('Repo', text('repo', s.repo)),
    field('Branch', text('branch', s.branch)),
    field('Directory', text('dir', s.dir)),
    tokenSection(hasToken, actions, status),
    field('Poll seconds', text('poll_seconds', String(s.poll_seconds), 'number')),
    field('Hotkey', text('hotkey', s.hotkey)),
    field('Dock', select('dock', s.dock, DOCKS)),
    field('Theme', select('theme', s.theme, THEMES)),
    field(`Columns (${MIN_COLUMNS}-${MAX_COLUMNS})`, text('columns', String(s.columns), 'number')),
    check('always_on_top', s.always_on_top, 'Always on top'),
    check('autostart', s.autostart, 'Start with Windows'),
    check('notify_new_tips', s.notify_new_tips, 'Toast when a new tip arrives'),
    check('show_done', s.show_done, 'Show done tiles'),
    status,
    el('div', { className: 'set-row' }, el('button', { type: 'submit', className: 'action action-primary', text: 'Save' }), el('button', { type: 'button', className: 'action action-back', text: 'Back', 'data-role': 'back' })),
  );
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    void (async () => {
      try {
        await actions.onSave(readSettings(form, s));
        status.textContent = 'Saved';
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
