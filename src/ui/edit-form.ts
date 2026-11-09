/**
 * In-app edit form for one tip (or a new one, in create mode), in the visual
 * language of the settings overlay. The backend validates again
 * (`core/edit.rs`); this side catches the obvious mistakes early and shows
 * every error inline.
 */
import type { Kind, Priority, Tip, TipEdit } from '../types';
import { kindLabel, t, type StringKey } from '../lib/i18n';
import { parseNaive } from '../lib/time';
import { el } from './dom';
import { errorMessage } from './errors';

/** Same limits as `src-tauri/src/core/edit.rs`. */
export const MAX_TITLE_CHARS = 200;
export const MAX_LOCATION_CHARS = 200;
export const MAX_BODY_CHARS = 100_000;
const BODY_ROWS = 10;
/** A second Esc within this long discards unsaved changes. */
export const DISCARD_CONFIRM_MS = 2000;
/** A clock time inside a due string: `09:30`, `9:30`, `T23:59`, `09:30:00`. */
const DUE_TIME_RE = /(?:^|[\sT])\d{1,2}:\d{2}(?::\d{2})?(?:\s*[ap]\.?m\.?)?\s*$/i;

const KINDS: readonly Kind[] = ['task', 'deadline', 'note', 'reading', 'event', 'agent'];
const PRIORITIES: readonly Priority[] = ['low', 'normal', 'high'];
/** Tile size overrides; '' in the select is the default (size by score). */
const SIZES = ['sm', 'md', 'wide'] as const;
/** Suggestions for the repeat field; any rule `core/recur.rs` accepts may be typed. */
export const REPEAT_SUGGESTIONS: readonly string[] = ['daily', 'weekdays', 'weekly on mon', 'monthly on 1', 'yearly on 03-15'];
const COLOR_RE = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;
/** `-1d`, `+2h`, `-30m`, `-1w`: an offset from the due time (`core/timeparse.rs`). */
const REMIND_OFFSET_RE = /^[-+]\d+[mhdw]$/;
/** `2026-10-13`, `2026/10/13 09:00`, `2026-10-13T09:00:00`. */
const REMIND_DATE_RE = /^\d{4}[-/]\d{2}[-/]\d{2}(?:[ T]\d{1,2}:\d{2}(?::\d{2})?)?$/;
const BARE_DATE_RE = /^\d{4}[-/]\d{2}[-/]\d{2}$/;
const BARE_TIME_RE = /^\d{1,2}:\d{2}(?::\d{2})?$/;

export interface EditFormActions {
  /** Persist the edit; a rejection is shown in the form, which stays open. */
  onSave: (edit: TipEdit) => Promise<void>;
  onCancel: () => void;
}

export type EditFormMode = 'edit' | 'create';

export interface EditFormOptions {
  tip: Tip;
  actions: EditFormActions;
  /** `create`: a new tip prefilled from `tip`; the submit button says Create. Default `edit`. */
  mode?: EditFormMode;
}

const pad = (n: number): string => String(n).padStart(2, '0');

/** A blank new tip: a task of normal priority with nothing else set. */
export function emptyDraft(): TipEdit {
  return {
    title: '',
    kind: 'task',
    priority: 'normal',
    due: null,
    location: null,
    tags: [],
    remind: [],
    repeat: null,
    links: [],
    images: [],
    color: null,
    size: null,
    highlight: [],
    body: '',
  };
}

/** A stand-in Tip for the create form, so a draft renders through the same fields as an edit. */
export function tipFromDraft(draft: TipEdit): Tip {
  const due = draft.due ?? undefined;
  const dueAt = due ? (/\d:\d{2}/.test(due) ? `${due.replace(' ', 'T')}:00` : `${due}T23:59:00`) : undefined;
  const list = (values: string[]): string[] | undefined => (values.length ? values : undefined);
  return {
    id: '',
    path: '',
    title: draft.title,
    kind: draft.kind,
    priority: draft.priority,
    status: 'open',
    due,
    due_at: dueAt,
    location: draft.location ?? undefined,
    tags: draft.tags,
    remind: list(draft.remind),
    repeat: draft.repeat ?? undefined,
    links: list(draft.links),
    images: list(draft.images),
    color: draft.color ?? undefined,
    size: draft.size ?? undefined,
    highlight: list(draft.highlight),
    body: draft.body,
    remind_at: [],
  };
}

/** The date and time inputs for a tip's due value; a bare date leaves the time empty. */
export function dueFields(tip: Tip): { date: string; time: string } {
  const due = parseNaive(tip.due_at);
  if (!due) return { date: '', time: '' };
  const date = `${due.getFullYear()}-${pad(due.getMonth() + 1)}-${pad(due.getDate())}`;
  // Without the raw string, keep the resolved time rather than drop it.
  const hasTime = tip.due === undefined || DUE_TIME_RE.test(tip.due.trim());
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

/** Non-empty trimmed lines. */
export function splitLines(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

/** Comma-separated phrases (spaces stay inside a phrase), trimmed and deduplicated. */
export function splitPhrases(text: string): string[] {
  const phrases = text
    .split(/[,，、]/)
    .map((p) => p.trim())
    .filter(Boolean);
  return Array.from(new Set(phrases));
}

/** Same rule as `core/edit.rs`: shorter highlight words would mark half the board. */
export const MIN_HIGHLIGHT_CHARS = 2;
export const MAX_HIGHLIGHT_CHARS = 64;

/**
 * Highlight phrases, deduplicated ignoring case (first spelling wins), each at
 * least `MIN_HIGHLIGHT_CHARS` long. Throws a user-facing Error.
 */
export function readHighlights(text: string): string[] {
  const out: string[] = [];
  for (const phrase of splitPhrases(text)) {
    if (out.some((w) => w.toLowerCase() === phrase.toLowerCase())) continue;
    if (Array.from(phrase).length < MIN_HIGHLIGHT_CHARS) throw new Error(`${phrase}: ${t('highlightTooShort')}`);
    if (Array.from(phrase).length > MAX_HIGHLIGHT_CHARS) throw new Error(`${phrase}: ${t('tooLong')} (≤ ${MAX_HIGHLIGHT_CHARS})`);
    out.push(phrase);
  }
  return out;
}

/** Remind entries as typed, separated by commas or spaces; `2026-10-13 09:00` stays one entry. */
function remindEntries(text: string): string[] {
  const out: string[] = [];
  for (const token of text.split(/[\s,，]+/).filter(Boolean)) {
    const last = out[out.length - 1];
    if (last !== undefined && BARE_DATE_RE.test(last) && BARE_TIME_RE.test(token)) out[out.length - 1] = `${last} ${token}`;
    else out.push(token);
  }
  return Array.from(new Set(out));
}

/**
 * Reminder entries separated by commas or spaces. A clock time right after a
 * bare date belongs to it (`2026-10-13 09:00` is one entry). Throws on an
 * entry shaped like neither an offset nor a date, and on an offset when
 * there is no due date for it to count from.
 */
export function splitRemind(text: string, hasDue = true): string[] {
  const out = remindEntries(text);
  for (const entry of out) {
    if (REMIND_OFFSET_RE.test(entry)) {
      if (!hasDue) throw new Error(`${entry}: ${t('remindNeedsDue')}`);
    } else if (!REMIND_DATE_RE.test(entry)) throw new Error(`${entry}: ${t('badRemind')}`);
  }
  return out;
}

/** `#rgb` / `#rrggbb` → `#rrggbb` in lower case (what the native picker takes); `undefined` if not a hex colour. */
export function pickerColor(value: string): string | undefined {
  const v = value.trim().toLowerCase();
  if (!COLOR_RE.test(v)) return undefined;
  return v.length === 4 ? `#${v[1]}${v[1]}${v[2]}${v[2]}${v[3]}${v[3]}` : v;
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

/**
 * Read the form into a validated `TipEdit`; throws a user-facing Error.
 * A field left as it was opened is not re-checked: like `core/edit.rs`, an
 * unchanged value keeps whatever the file says (a CSS colour name, a
 * hand-written reminder), so it must not block saving other changes.
 */
export function readEditForm(form: HTMLFormElement): TipEdit {
  const data = new FormData(form);
  const raw = (name: string): string => String(data.get(name) ?? '');
  const untouched = (name: string): boolean => {
    const node = form.querySelector<HTMLElement>(`[name="${name}"]`);
    return node?.dataset.original !== undefined && node.dataset.original === raw(name);
  };
  const title = raw('title').trim();
  if (!title) throw new Error(t('titleRequired'));
  const date = raw('due_date').trim();
  const time = raw('due_time').trim();
  if (time && !date) throw new Error(t('timeNeedsDate'));
  const location = raw('location').trim();
  const color = raw('color').trim();
  if (color && !COLOR_RE.test(color) && !untouched('color')) throw new Error(`${color}: ${t('badColor')}`);
  const size = raw('size').trim();
  const repeat = raw('repeat').trim();
  const remind = untouched('remind') ? remindEntries(raw('remind')) : splitRemind(raw('remind'), !!date);
  return {
    title: limited('fieldTitle', title, MAX_TITLE_CHARS),
    kind: oneOf('fieldKind', raw('kind'), KINDS),
    priority: oneOf('fieldPriority', raw('priority'), PRIORITIES),
    due: date ? (time ? `${date} ${time}` : date) : null,
    location: location ? limited('fieldLocation', location, MAX_LOCATION_CHARS) : null,
    tags: splitTags(raw('tags')),
    remind,
    repeat: repeat || null,
    links: splitLines(raw('links')),
    images: splitLines(raw('images')),
    color: color || null,
    size: size && !untouched('size') ? oneOf('fieldSize', size, SIZES) : size || null,
    highlight: untouched('highlight') ? splitPhrases(raw('highlight')) : readHighlights(raw('highlight')),
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

const textInput = (value: string, extra: Record<string, string> = {}): HTMLInputElement =>
  el('input', { type: 'text', value, autocomplete: 'off', ...extra });

/** Hex text box plus a native colour picker, kept in sync; blank text = no override. */
function colorField(value: string): HTMLElement {
  const text = textInput(value, { spellcheck: 'false', placeholder: '#4a3b6b', 'data-original': value });
  // No name: the picker is a way to fill the text box, not a second value.
  const picker = el('input', { type: 'color', className: 'set-color-picker', 'aria-label': t('fieldColor'), tabindex: '-1' });
  const sync = (): void => {
    const hex = pickerColor(text.value);
    if (hex) picker.value = hex;
    picker.dataset.empty = String(!hex);
  };
  sync();
  text.addEventListener('input', sync);
  picker.addEventListener('input', () => {
    text.value = picker.value;
    picker.dataset.empty = 'false';
  });
  const labelled = field('fieldColor', 'color', text);
  labelled.append(el('span', { className: 'set-color-row' }, text, picker));
  return labelled;
}

/** The less common fields, folded under "More"; open when any of them has a value. */
function moreFields(tip: Tip): HTMLDetailsElement {
  const lines = (values: string[] | undefined): HTMLTextAreaElement => {
    const area = el('textarea', { rows: '2', spellcheck: 'false', className: 'set-lines' });
    area.value = (values ?? []).join('\n');
    return area;
  };
  const repeatOptions = el('datalist', { id: 'edit-repeat-options' });
  for (const rule of REPEAT_SUGGESTIONS) repeatOptions.append(el('option', { value: rule }));
  const size = el('select', { 'data-original': tip.size ?? '' });
  size.append(el('option', { value: '', selected: !tip.size, text: t('sizeDefault') }));
  const sizeLabels = { sm: t('sizeSm'), md: t('sizeMd'), wide: t('sizeWide') } as const;
  for (const s of SIZES) size.append(el('option', { value: s, selected: s === tip.size, text: sizeLabels[s] }));
  // An unknown size from a hand-edited file stays visible (and is rejected on save) rather than silently cleared.
  if (tip.size && !SIZES.some((s) => s === tip.size)) size.append(el('option', { value: tip.size, selected: true, text: tip.size }));
  const remindText = (tip.remind ?? []).join(', ');
  const highlightText = (tip.highlight ?? []).join(', ');
  const hasValue = [tip.remind, tip.links, tip.images, tip.highlight].some((list) => (list?.length ?? 0) > 0) || !!(tip.repeat || tip.color || tip.size);
  return el(
    'details',
    { className: 'set-more', open: hasValue },
    el('summary', { className: 'set-more-summary', text: t('moreOptions') }),
    el(
      'div',
      { className: 'set-more-body' },
      field('fieldRemind', 'remind', textInput(remindText, { spellcheck: 'false', placeholder: '-1d -2h 2026-10-13 09:00', 'data-original': remindText })),
      field('fieldRepeat', 'repeat', textInput(tip.repeat ?? '', { spellcheck: 'false', list: 'edit-repeat-options' })),
      repeatOptions,
      el('div', { className: 'set-pair' }, colorField(tip.color ?? ''), field('fieldSize', 'size', size)),
      field('fieldHighlight', 'highlight', textInput(highlightText, { 'data-original': highlightText })),
      field('fieldLinks', 'links', lines(tip.links)),
      field('fieldImages', 'images', lines(tip.images)),
    ),
  );
}

function fields(tip: Tip): HTMLElement[] {
  const due = dueFields(tip);
  const body = el('textarea', { rows: String(BODY_ROWS), spellcheck: 'false' });
  body.value = tip.body;
  return [
    field('fieldTitle', 'title', textInput(tip.title, { maxlength: String(MAX_TITLE_CHARS), required: 'required' })),
    el(
      'div',
      { className: 'set-pair' },
      field('fieldKind', 'kind', select(tip.kind, KINDS, (k) => kindLabel(k))),
      field('fieldPriority', 'priority', select(tip.priority, PRIORITIES, (p) => t(`prio_${p}`))),
    ),
    el(
      'div',
      { className: 'set-pair' },
      field('fieldDueDate', 'due_date', textInput(due.date, { type: 'date' })),
      field('fieldDueTime', 'due_time', textInput(due.time, { type: 'time' })),
    ),
    field('fieldLocation', 'location', textInput(tip.location ?? '', { maxlength: String(MAX_LOCATION_CHARS) })),
    field('fieldTags', 'tags', textInput((tip.tags ?? []).join(' '))),
    moreFields(tip),
    field('fieldBody', 'body', body),
  ];
}

/** What the form opened with, per form, for `isEditFormDirty`. */
const originals = new WeakMap<HTMLFormElement, string>();

function formSnapshot(form: HTMLFormElement): string {
  return JSON.stringify(Array.from(new FormData(form).entries(), ([k, v]) => [k, String(v)]));
}

/** The form holds changes a cancel would throw away (unknown forms count as clean). */
export function isEditFormDirty(form: HTMLFormElement): boolean {
  const original = originals.get(form);
  return original !== undefined && formSnapshot(form) !== original;
}

/** Show `text` as a lasting notice in the form's status line. */
export function showEditFormNotice(form: HTMLFormElement, text: string): void {
  const status = form.querySelector<HTMLElement>('.set-status');
  if (!status) return;
  status.textContent = text;
  status.dataset.state = 'error';
}

/** Render the edit form for `tip` (create mode: a new tip prefilled from it). Esc cancels, Ctrl+Enter saves. */
export function renderEditForm(options: EditFormOptions): HTMLFormElement {
  const { tip, actions } = options;
  const creating = options.mode === 'create';
  const heading = creating ? t('newTip') : t('editTip');
  const status = el('p', { className: 'set-status', role: 'status' });
  const save = el('button', { type: 'submit', className: 'action action-primary', text: creating ? t('create') : t('save') });
  const cancel = el('button', { type: 'button', className: 'action action-back', text: t('cancel') });
  const form = el(
    'form',
    { className: 'settings edit-form', 'aria-label': heading, 'data-editing': creating ? 'new' : tip.id, novalidate: true },
    el('h1', { className: 'set-title', text: heading }),
    ...fields(tip),
    el('p', { className: 'set-hint', text: t('editHint') }),
    status,
    el('div', { className: 'set-row set-actions' }, save, cancel),
  );
  const report = (error: unknown): void => {
    status.textContent = errorMessage(error);
    status.dataset.state = 'error';
  };
  // Everything a cancel would throw away, compared against what the form
  // opened with: the tip, or in create mode the prefilled draft.
  const snapshot = (): string => formSnapshot(form);
  const original = snapshot();
  originals.set(form, original);
  let armedUntil = 0;
  let armTimer: ReturnType<typeof setTimeout> | undefined;
  const disarm = (): void => {
    armedUntil = 0;
    if (armTimer) clearTimeout(armTimer);
    armTimer = undefined;
    if (status.dataset.state === 'confirm') {
      status.textContent = '';
      status.dataset.state = '';
    }
  };
  /** Esc: leave at once when nothing changed, else only on a second Esc within the window. */
  const escape = (): void => {
    if (snapshot() === original || Date.now() < armedUntil) {
      disarm();
      actions.onCancel();
      return;
    }
    armedUntil = Date.now() + DISCARD_CONFIRM_MS;
    status.textContent = t('discardHint');
    status.dataset.state = 'confirm';
    if (armTimer) clearTimeout(armTimer);
    armTimer = setTimeout(disarm, DISCARD_CONFIRM_MS);
  };
  // One save at a time: `requestSubmit` (Ctrl+Enter) ignores the disabled button.
  let busy = false;
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    if (busy) return;
    let edit: TipEdit;
    try {
      edit = readEditForm(form);
    } catch (error) {
      report(error);
      return;
    }
    busy = true;
    save.disabled = true;
    actions
      .onSave(edit)
      .catch(report)
      .finally(() => {
        busy = false;
        save.disabled = false;
      });
  });
  form.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      // The form owns Esc: the board's own Esc handler must not close the detail underneath.
      event.stopPropagation();
      escape();
    } else if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      form.requestSubmit();
    }
  });
  // The Cancel button is a deliberate click, so it discards without asking.
  cancel.addEventListener('click', () => {
    disarm();
    actions.onCancel();
  });
  return form;
}
