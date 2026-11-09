import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Tip, TipEdit } from '../types';
import { setLocale } from '../lib/i18n';
import {
  DISCARD_CONFIRM_MS,
  dueFields,
  emptyDraft,
  isEditFormDirty,
  pickerColor,
  readEditForm,
  readHighlights,
  renderEditForm,
  splitLines,
  splitRemind,
  splitTags,
  tipFromDraft,
  type EditFormActions,
  type EditFormOptions,
} from './edit-form';

afterEach(() => setLocale('en'));

const base: Tip = {
  id: 'camera',
  path: 'tips/camera.md',
  sha: 'stamp-1',
  title: 'Submit camera-ready',
  kind: 'deadline',
  priority: 'high',
  status: 'open',
  due: '2026-10-15',
  due_at: '2026-10-15T23:59:00',
  location: 'Lab 302',
  tags: ['ecce', 'paper'],
  links: ['https://example.org'],
  body: '\nOld body.\n',
  remind_at: [],
};

function setup(tip: Tip = base, onSave: EditFormActions['onSave'] = vi.fn(async () => undefined), mode: EditFormOptions['mode'] = 'edit') {
  const onCancel = vi.fn();
  const form = renderEditForm({ tip, actions: { onSave, onCancel }, mode });
  document.body.append(form);
  const field = <T extends HTMLElement>(name: string): T => {
    const node = form.querySelector<T>(`[name="${name}"]`);
    if (!node) throw new Error(`missing ${name}`);
    return node;
  };
  return { form, onSave, onCancel, field };
}

describe('dueFields', () => {
  it('splits a due value into the date and time inputs', () => {
    expect(dueFields(base)).toEqual({ date: '2026-10-15', time: '' });
    expect(dueFields({ ...base, due: '2026-10-15 08:30', due_at: '2026-10-15T08:30:00' })).toEqual({ date: '2026-10-15', time: '08:30' });
    expect(dueFields({ ...base, due: '2026/10/15', due_at: '2026-10-15T23:59:00' })).toEqual({ date: '2026-10-15', time: '' });
    expect(dueFields({ ...base, due: undefined, due_at: undefined })).toEqual({ date: '', time: '' });
  });
  it('decides "has a time" from a clock time in the due string, not any colon', () => {
    const at = (due: string, dueAt: string) => dueFields({ ...base, due, due_at: dueAt });
    expect(at('2026-10-15T08:30', '2026-10-15T08:30:00')).toEqual({ date: '2026-10-15', time: '08:30' });
    expect(at('2026-10-15 8:05', '2026-10-15T08:05:00')).toEqual({ date: '2026-10-15', time: '08:05' });
    expect(at('2026-10-15 08:30:00', '2026-10-15T08:30:00')).toEqual({ date: '2026-10-15', time: '08:30' });
    expect(at('tomorrow 9:30', '2026-10-15T09:30:00')).toEqual({ date: '2026-10-15', time: '09:30' });
    // A colon that is not a clock time does not invent one.
    expect(at('due: 2026-10-15', '2026-10-15T23:59:00')).toEqual({ date: '2026-10-15', time: '' });
    expect(at('2026-10-15 ', '2026-10-15T23:59:00')).toEqual({ date: '2026-10-15', time: '' });
  });
});

describe('splitTags', () => {
  it('accepts spaces, commas and # signs', () => {
    expect(splitTags(' #ecce, paper，理论  ecce ')).toEqual(['ecce', 'paper', '理论']);
    expect(splitTags('   ')).toEqual([]);
  });
});

describe('readEditForm', () => {
  it('reads the prefilled form back unchanged', () => {
    const { form } = setup();
    const expected: TipEdit = {
      title: 'Submit camera-ready',
      kind: 'deadline',
      priority: 'high',
      due: '2026-10-15',
      location: 'Lab 302',
      tags: ['ecce', 'paper'],
      remind: [],
      repeat: null,
      links: ['https://example.org'],
      images: [],
      color: null,
      size: null,
      highlight: [],
      body: '\nOld body.\n',
    };
    expect(readEditForm(form)).toEqual(expected);
  });

  it('combines date and time and clears an empty due', () => {
    const { form, field } = setup();
    field<HTMLInputElement>('due_time').value = '17:00';
    expect(readEditForm(form).due).toBe('2026-10-15 17:00');
    field<HTMLInputElement>('due_date').value = '';
    field<HTMLInputElement>('due_time').value = '';
    expect(readEditForm(form).due).toBeNull();
    field<HTMLInputElement>('location').value = '  ';
    expect(readEditForm(form).location).toBeNull();
  });

  it('rejects a missing title, a time without a date and over-long fields', () => {
    const { form, field } = setup();
    field<HTMLInputElement>('title').value = '   ';
    expect(() => readEditForm(form)).toThrow(/title/);
    field<HTMLInputElement>('title').value = 'ok';
    field<HTMLInputElement>('due_date').value = '';
    field<HTMLInputElement>('due_time').value = '09:00';
    expect(() => readEditForm(form)).toThrow(/date/);
    field<HTMLInputElement>('due_time').value = '';
    field<HTMLInputElement>('title').value = 'x'.repeat(201);
    expect(() => readEditForm(form)).toThrow(/too long/);
  });

  it('rejects a kind or priority outside the known set', () => {
    const { form, field } = setup();
    const kind = field<HTMLSelectElement>('kind');
    kind.append(new Option('chore', 'chore'));
    kind.value = 'chore';
    expect(() => readEditForm(form)).toThrow(/kind/);
  });
});

describe('renderEditForm', () => {
  it('saves on submit and on Ctrl+Enter, cancels on Esc', async () => {
    const { form, onSave, onCancel, field } = setup();
    field<HTMLInputElement>('title').value = 'Renamed';
    form.requestSubmit();
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(vi.mocked(onSave).mock.calls[0]?.[0].title).toBe('Renamed');
    // Wait for the first save to settle: a submit while one is in flight is ignored.
    await vi.waitFor(() => expect(form.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(false));
    field<HTMLTextAreaElement>('body').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(2));
    // The title was changed, so the first Esc only arms the discard; the second leaves.
    const esc = (): void => void field<HTMLInputElement>('title').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    esc();
    expect(onCancel).not.toHaveBeenCalled();
    esc();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('ignores Ctrl+Enter while a save is in flight', async () => {
    let finish: () => void = () => undefined;
    const onSave = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    const { field } = setup(base, onSave);
    const ctrlEnter = (): void =>
      void field<HTMLTextAreaElement>('body').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }));
    ctrlEnter();
    ctrlEnter();
    ctrlEnter();
    expect(onSave).toHaveBeenCalledTimes(1);
    finish();
    await vi.waitFor(() => expect(field<HTMLTextAreaElement>('body').form?.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(false));
    ctrlEnter();
    expect(onSave).toHaveBeenCalledTimes(2);
  });

  it('reports unsaved changes through isEditFormDirty', () => {
    const { form, field } = setup();
    expect(isEditFormDirty(form)).toBe(false);
    field<HTMLInputElement>('title').value = 'changed';
    expect(isEditFormDirty(form)).toBe(true);
  });

  it('cancels on a single Esc when nothing changed, without bubbling to the board', () => {
    const { form, onCancel, field } = setup();
    const bubble = vi.fn();
    document.addEventListener('keydown', bubble);
    field<HTMLInputElement>('title').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    document.removeEventListener('keydown', bubble);
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(bubble).not.toHaveBeenCalled();
    expect(form.querySelector('.set-status')?.textContent).toBe('');
  });

  it('asks for a second Esc within the window before discarding edits', () => {
    vi.useFakeTimers();
    try {
      const { form, onCancel, field } = setup();
      const esc = (): void => void field<HTMLTextAreaElement>('body').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      field<HTMLTextAreaElement>('body').value = 'half-typed thought';
      esc();
      expect(onCancel).not.toHaveBeenCalled();
      const status = form.querySelector<HTMLElement>('.set-status');
      expect(status?.textContent).toMatch(/Esc again/);
      expect(status?.dataset.state).toBe('confirm');
      // Too slow: the hint clears and the next Esc only arms again.
      vi.advanceTimersByTime(DISCARD_CONFIRM_MS + 1);
      expect(status?.textContent).toBe('');
      esc();
      expect(onCancel).not.toHaveBeenCalled();
      vi.advanceTimersByTime(DISCARD_CONFIRM_MS - 500);
      esc();
      expect(onCancel).toHaveBeenCalledTimes(1);
      expect(field<HTMLTextAreaElement>('body').value).toBe('half-typed thought');
    } finally {
      vi.useRealTimers();
    }
  });

  it('treats a change put back to the original as clean', () => {
    const { onCancel, field } = setup();
    const title = field<HTMLInputElement>('title');
    title.value = 'something else';
    title.value = base.title;
    title.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('the Cancel button discards without asking', () => {
    const { form, onCancel, field } = setup();
    field<HTMLInputElement>('title').value = 'changed';
    form.querySelector<HTMLButtonElement>('.action-back')?.click();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('shows validation errors inline without calling save', async () => {
    const { form, onSave, field } = setup();
    field<HTMLInputElement>('title').value = '';
    form.requestSubmit();
    await vi.waitFor(() => expect(form.querySelector('.set-status')?.textContent).toMatch(/title/));
    expect(form.querySelector<HTMLElement>('.set-status')?.dataset.state).toBe('error');
    expect(onSave).not.toHaveBeenCalled();
  });

  it('keeps the typed text when saving fails (e.g. a conflict)', async () => {
    const onSave = vi.fn(async () => {
      throw new Error('update_tip: tips/camera.md changed on disk since it was loaded; reloaded, please retry');
    });
    const { form, field } = setup(base, onSave);
    field<HTMLTextAreaElement>('body').value = 'my careful notes';
    form.requestSubmit();
    await vi.waitFor(() => expect(form.querySelector('.set-status')?.textContent).toMatch(/changed on disk/));
    expect(field<HTMLTextAreaElement>('body').value).toBe('my careful notes');
    expect(form.querySelector<HTMLButtonElement>('button[type="submit"]')?.disabled).toBe(false);
  });

  it('is labelled in the UI language', () => {
    setLocale('zh');
    const { form } = setup();
    expect(form.querySelector('.set-title')?.textContent).toBe('编辑事项');
  });
});

describe('list helpers', () => {
  it('splits reminders on commas and spaces, keeping a date and its time together', () => {
    expect(splitRemind('-1d, -2h 2026-10-13 09:00，2026/10/14 -1d')).toEqual(['-1d', '-2h', '2026-10-13 09:00', '2026/10/14']);
    expect(splitRemind('  ')).toEqual([]);
    expect(splitRemind('2026-10-13T09:00')).toEqual(['2026-10-13T09:00']);
  });
  it('rejects reminders shaped like neither an offset nor a date, and offsets without a due', () => {
    expect(() => splitRemind('-1d soon')).toThrow(/soon: not a reminder/);
    expect(() => splitRemind('09:00')).toThrow(/not a reminder/);
    expect(() => splitRemind('-1y')).toThrow(/not a reminder/);
    expect(() => splitRemind('-1d', false)).toThrow(/due date/);
    expect(splitRemind('2026-10-13 09:00', false)).toEqual(['2026-10-13 09:00']);
  });
  it('splits lines and highlight phrases without empty entries', () => {
    expect(splitLines(' a.png \r\n\n https://x.org/b.svg\n')).toEqual(['a.png', 'https://x.org/b.svg']);
    expect(readHighlights('Reviewer 2, reviewer 2，ORCID , ')).toEqual(['Reviewer 2', 'ORCID']);
    expect(() => readHighlights('a, ORCID')).toThrow(/too short/);
  });
  it('normalises hex colours for the native picker', () => {
    expect(pickerColor('#ABC')).toBe('#aabbcc');
    expect(pickerColor(' #4A3B6B ')).toBe('#4a3b6b');
    expect(pickerColor('teal')).toBeUndefined();
    expect(pickerColor('#12345')).toBeUndefined();
  });
});

describe('the More fields', () => {
  const plain: Tip = { ...base, links: undefined };
  const full: Tip = {
    ...base,
    remind: ['-1d', '2026-10-13 09:00'],
    repeat: 'weekly on mon',
    links: ['https://a.org', 'https://b.org'],
    images: ['figures/a.png'],
    color: '#4A3B6B',
    size: 'wide',
    highlight: ['EDAS', '250 words'],
  };
  const details = (tip: Tip): HTMLDetailsElement | null => setup(tip).form.querySelector<HTMLDetailsElement>('details.set-more');

  it('stay folded when none has a value and open when one does', () => {
    expect(details(plain)?.open).toBe(false);
    expect(details(base)?.open).toBe(true);
    expect(details({ ...plain, color: '#123456' })?.open).toBe(true);
  });

  it('are prefilled and read back unchanged', () => {
    const { form, field } = setup(full);
    expect(field<HTMLInputElement>('remind').value).toBe('-1d, 2026-10-13 09:00');
    expect(field<HTMLTextAreaElement>('links').value).toBe('https://a.org\nhttps://b.org');
    expect(field<HTMLSelectElement>('size').value).toBe('wide');
    expect(readEditForm(form)).toMatchObject({
      remind: ['-1d', '2026-10-13 09:00'],
      repeat: 'weekly on mon',
      links: ['https://a.org', 'https://b.org'],
      images: ['figures/a.png'],
      color: '#4A3B6B',
      size: 'wide',
      highlight: ['EDAS', '250 words'],
    });
  });

  it('offers repeat suggestions and the size choices', () => {
    const { form, field } = setup(plain);
    const list = field<HTMLInputElement>('repeat').getAttribute('list') ?? '';
    const options = Array.from(form.querySelectorAll<HTMLOptionElement>(`datalist#${list} option`), (o) => o.value);
    expect(options).toEqual(['daily', 'weekdays', 'weekly on mon', 'monthly on 1', 'yearly on 03-15']);
    expect(Array.from(field<HTMLSelectElement>('size').options, (o) => o.value)).toEqual(['', 'sm', 'md', 'wide']);
  });

  it('clears blank fields to null or empty lists', () => {
    const { form, field } = setup(full);
    for (const name of ['remind', 'repeat', 'color', 'highlight']) field<HTMLInputElement>(name).value = '  ';
    for (const name of ['links', 'images']) field<HTMLTextAreaElement>(name).value = '\n \n';
    field<HTMLSelectElement>('size').value = '';
    expect(readEditForm(form)).toMatchObject({ remind: [], repeat: null, links: [], images: [], color: null, size: null, highlight: [] });
  });

  it('rejects a changed colour that is not hex, a bad reminder and an unknown size', () => {
    const { form, field } = setup(plain);
    field<HTMLInputElement>('color').value = 'red';
    expect(() => readEditForm(form)).toThrow(/red: not a colour/);
    field<HTMLInputElement>('color').value = '#abc';
    expect(readEditForm(form).color).toBe('#abc');
    field<HTMLInputElement>('remind').value = '-1d tomorrow';
    expect(() => readEditForm(form)).toThrow(/tomorrow: not a reminder/);
    field<HTMLInputElement>('remind').value = '-1d';
    field<HTMLInputElement>('due_date').value = '';
    expect(() => readEditForm(form)).toThrow(/due date/);
    field<HTMLInputElement>('remind').value = '';
    const size = field<HTMLSelectElement>('size');
    size.append(new Option('huge', 'huge'));
    size.value = 'huge';
    expect(() => readEditForm(form)).toThrow(/size/i);
  });

  it('lets hand-written values the form would not accept pass when left untouched', () => {
    const { form } = setup({ ...plain, color: 'teal', remind: ['tomorrow 9am'], size: 'huge' });
    expect(readEditForm(form)).toMatchObject({ color: 'teal', remind: ['tomorrow', '9am'], size: 'huge' });
  });

  it('keeps the text box and the native colour picker in sync', () => {
    const { form, field } = setup(plain);
    const text = field<HTMLInputElement>('color');
    const picker = form.querySelector<HTMLInputElement>('input[type="color"]');
    if (!picker) throw new Error('no picker');
    expect(picker.dataset.empty).toBe('true');
    expect(picker.name).toBe('');
    text.value = '#ABC';
    text.dispatchEvent(new Event('input'));
    expect(picker.value).toBe('#aabbcc');
    expect(picker.dataset.empty).toBe('false');
    picker.value = '#102030';
    picker.dispatchEvent(new Event('input'));
    expect(text.value).toBe('#102030');
    expect(readEditForm(form).color).toBe('#102030');
  });
});

describe('create mode', () => {
  const draft = { ...emptyDraft(), title: 'Check coil', priority: 'high' as const, tags: ['lab'], due: '2026-10-15 09:30' };

  it('says Create, marks the form as new and reads back the draft', () => {
    const { form } = setup(tipFromDraft(draft), undefined, 'create');
    expect(form.dataset.editing).toBe('new');
    expect(form.querySelector('button[type="submit"]')?.textContent).toBe('Create');
    expect(form.querySelector('.set-title')?.textContent).toBe('New tip');
    expect(readEditForm(form)).toEqual(draft);
  });

  it('reads back a bare-date draft; an empty draft still needs a title', () => {
    const bare = { ...draft, due: '2026-10-15' };
    expect(readEditForm(setup(tipFromDraft(bare), undefined, 'create').form)).toEqual(bare);
    const empty = setup(tipFromDraft(emptyDraft()), undefined, 'create');
    expect(() => readEditForm(empty.form)).toThrow(/title/);
  });

  it('compares the dirty check against the prefilled draft', () => {
    const { onCancel, field } = setup(tipFromDraft(draft), undefined, 'create');
    const esc = (): void => void field<HTMLInputElement>('title').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    esc();
    expect(onCancel).toHaveBeenCalledTimes(1);
    field<HTMLInputElement>('location').value = 'Lab 302';
    esc();
    expect(onCancel).toHaveBeenCalledTimes(1);
    esc();
    expect(onCancel).toHaveBeenCalledTimes(2);
  });

  it('is labelled in Chinese too', () => {
    setLocale('zh');
    const { form } = setup(tipFromDraft(draft), undefined, 'create');
    expect(form.querySelector('button[type="submit"]')?.textContent).toBe('创建');
    expect(form.querySelector('.set-more-summary')?.textContent).toBe('更多');
  });
});

describe('agent tips in the form', () => {
  const agentTip: Tip = {
    ...base,
    id: 'survey',
    path: 'tips/survey.md',
    title: 'Survey hairpin AC loss papers',
    kind: 'agent',
    due: undefined,
    due_at: undefined,
    body: 'Prompt\n',
    agent: { v: 1, state: 'done', hash: 'sha256:x', zz: 'keep' },
    agent_state: 'done',
  };
  const kindOptions = (form: HTMLFormElement): [string, string][] =>
    Array.from(form.querySelector<HTMLSelectElement>('[name="kind"]')?.options ?? []).map((o) => [o.value, o.textContent ?? '']);

  it('the kind selector offers agent, labelled in English and Chinese, in both modes', () => {
    for (const mode of ['edit', 'create'] as const) {
      setLocale('en');
      expect(kindOptions(setup(agentTip, undefined, mode).form)).toContainEqual(['agent', 'Agent']);
      setLocale('zh');
      expect(kindOptions(setup(agentTip, undefined, mode).form)).toContainEqual(['agent', '智能体']);
    }
  });

  it('the Edit form has no agent field and never sends one', () => {
    const { form, field } = setup(agentTip);
    expect(field<HTMLSelectElement>('kind').value).toBe('agent');
    const names = Array.from(form.elements, (e) => (e as HTMLInputElement).name).filter(Boolean);
    expect(names.some((n) => /agent/i.test(n))).toBe(false);
    const edit = readEditForm(form);
    expect(edit.kind).toBe('agent');
    expect(edit.body).toBe('Prompt\n');
    expect(Object.keys(edit)).not.toContain('agent');
    expect(Object.keys(edit)).not.toContain('agent_state');
    // Switching the kind away does not send anything about the map either.
    field<HTMLSelectElement>('kind').value = 'task';
    expect(Object.keys(readEditForm(form))).not.toContain('agent');
  });

  it('the new-tip form makes an agent tip from the kind and the typed text only', () => {
    const { form, field } = setup(tipFromDraft({ ...emptyDraft(), title: 'Survey papers' }), undefined, 'create');
    field<HTMLSelectElement>('kind').value = 'agent';
    field<HTMLTextAreaElement>('body').value = 'Find three recent papers.';
    expect(readEditForm(form)).toEqual({ ...emptyDraft(), title: 'Survey papers', kind: 'agent', body: 'Find three recent papers.' });
  });
});
