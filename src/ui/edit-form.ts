/**
 * In-app edit form for one tip, in the visual language of the settings
 * overlay. The backend validates again (`core/edit.rs`); this side catches
 * the obvious mistakes early and shows every error inline.
 */
import type { Kind, Priority, Tip, TipEdit } from '../types';
import { kindLabel, t, type StringKey } from '../lib/i18n';
import { parseNaive } from '../lib/time';
import { el } from './dom';

/** Same limits as `src-tauri/src/core/edit.rs`. */
export const MAX_TITLE_CHARS = 200;
export const MAX_LOCATION_CHARS = 200;
export const MAX_BODY_CHARS = 100_000;
const BODY_ROWS = 10;

const KINDS: readonly Kind[] = ['task', 'deadline', 'note', 'reading', 'event'];
const PRIORITIES: readonly Priority[] = ['low', 'normal', 'high'];

export interface EditFormActions {
  /** Persist the edit; a rejection is shown in the form, which stays open. */
  onSave: (edit: TipEdit) => Promise<void>;
  onCancel: () => void;
}

export interface EditFormOptions {
  tip: Tip;
  actions: EditFormActions;
}

const pad = (n: number): string => String(n).padStart(2, '0');

/** The date and time inputs for a tip's due value; a bare date leaves the time empty. */
export function dueFields(tip: Tip): { date: string; time: string } {
  const due = parseNaive(tip.due_at);
  if (!due) return { date: '', time: '' };
  const date = `${due.getFullYear()}-${pad(due.getMonth() + 1)}-${pad(due.getDate())}`;
  // Without the raw string, keep the resolved time rather than drop it.
  const hasTime = tip.due === undefined || tip.due.includes(':');
  return { date, time: hasTime ? `${pad(due.getHours())}:${pad(due.getMinutes())}` : '' };
}

/** `#a, b，c  a` → `['a', 'b', 'c']`. */
export function splitTags(text: string): string[] {
  const tags = text
    .split(/[\s,，]+/)
    .map((tag) => tag.replace(/^[#＃]+/, '').trim())
    .filter(Boolean);
  return Array.from(new Set(tags));
}

function limited(label: StringKey, value: string, max: number): string {
  if (value.length > max) throw new Error(`${t(label)} ${t('tooLong')} (≤ ${max})`);
  return value;
}

function oneOf<T extends string>(label: StringKey, value: string, allowed: readonly T[]): T {
  const found = allowed.find((x) => x === value);
  if (!found) throw new Error(`${t('badChoice')} ${t(label).toLowerCase()}: ${value}`);
  return found;
}

/** Read the form into a validated `TipEdit`; throws a user-facing Error. */
export function readEditForm(form: HTMLFormElement): TipEdit {
  const data = new FormData(form);
  const raw = (name: string): string => String(data.get(name) ?? '');
  const title = raw('title').trim();
  if (!title) throw new Error(t('titleRequired'));
  const date = raw('due_date').trim();
  const time = raw('due_time').trim();
  if (time && !date) throw new Error(t('timeNeedsDate'));
  const location = raw('location').trim();
  return {
    title: limited('fieldTitle', title, MAX_TITLE_CHARS),
    kind: oneOf('fieldKind', raw('kind'), KINDS),
    priority: oneOf('fieldPriority', raw('priority'), PRIORITIES),
    due: date ? (time ? `${date} ${time}` : date) : null,
    location: location ? limited('fieldLocation', location, MAX_LOCATION_CHARS) : null,
    tags: splitTags(raw('tags')),
    body: limited('fieldBody', raw('body'), MAX_BODY_CHARS),
  };
}

function field(label: StringKey, name: string, input: HTMLElement): HTMLElement {
  const id = `edit-${name}`;
  input.setAttribute('id', id);
  input.setAttribute('name', name);
  return el('label', { className: 'set-field', for: id }, el('span', { className: 'set-label', text: t(label) }), input);
}

function select<T extends string>(value: T, options: readonly T[], label: (o: T) => string): HTMLSelectElement {
  const sel = el('select');
  for (const o of options) sel.append(el('option', { value: o, selected: o === value, text: label(o) }));
  return sel;
}

function fields(tip: Tip): HTMLElement[] {
  const due = dueFields(tip);
  const input = (value: string, extra: Record<string, string> = {}): HTMLInputElement =>
    el('input', { type: 'text', value, autocomplete: 'off', ...extra });
  const body = el('textarea', { rows: String(BODY_ROWS), spellcheck: 'false' });
  body.value = tip.body;
  return [
    field('fieldTitle', 'title', input(tip.title, { maxlength: String(MAX_TITLE_CHARS), required: 'required' })),
    el(
      'div',
      { className: 'set-pair' },
      field('fieldKind', 'kind', select(tip.kind, KINDS, (k) => kindLabel(k))),
      field('fieldPriority', 'priority', select(tip.priority, PRIORITIES, (p) => t(`prio_${p}`))),
    ),
    el(
      'div',
      { className: 'set-pair' },
      field('fieldDueDate', 'due_date', input(due.date, { type: 'date' })),
      field('fieldDueTime', 'due_time', input(due.time, { type: 'time' })),
    ),
    field('fieldLocation', 'location', input(tip.location ?? '', { maxlength: String(MAX_LOCATION_CHARS) })),
    field('fieldTags', 'tags', input((tip.tags ?? []).join(' '))),
    field('fieldBody', 'body', body),
  ];
}

/** Render the edit form for `tip`. Esc cancels, Ctrl+Enter saves. */
export function renderEditForm(options: EditFormOptions): HTMLFormElement {
  const { tip, actions } = options;
  const status = el('p', { className: 'set-status', role: 'status' });
  const save = el('button', { type: 'submit', className: 'action action-primary', text: t('save') });
  const cancel = el('button', { type: 'button', className: 'action action-back', text: t('cancel') });
  const form = el(
    'form',
    { className: 'settings edit-form', 'aria-label': t('editTip'), 'data-editing': tip.id, novalidate: true },
    el('h1', { className: 'set-title', text: t('editTip') }),
    ...fields(tip),
    el('p', { className: 'set-hint', text: t('editHint') }),
    status,
    el('div', { className: 'set-row set-actions' }, save, cancel),
  );
  const report = (error: unknown): void => {
    status.textContent = error instanceof Error ? error.message : String(error);
    status.dataset.state = 'error';
  };
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    let edit: TipEdit;
    try {
      edit = readEditForm(form);
    } catch (error) {
      report(error);
      return;
    }
    save.disabled = true;
    actions
      .onSave(edit)
      .catch(report)
      .finally(() => (save.disabled = false));
  });
  form.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      actions.onCancel();
    } else if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      form.requestSubmit();
    }
  });
  cancel.addEventListener('click', actions.onCancel);
  return form;
}
