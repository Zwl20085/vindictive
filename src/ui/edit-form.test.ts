import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Tip, TipEdit } from '../types';
import { setLocale } from '../lib/i18n';
import { DISCARD_CONFIRM_MS, dueFields, readEditForm, renderEditForm, splitTags, type EditFormActions } from './edit-form';

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

function setup(tip: Tip = base, onSave: EditFormActions['onSave'] = vi.fn(async () => undefined)) {
  const onCancel = vi.fn();
  const form = renderEditForm({ tip, actions: { onSave, onCancel } });
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
    field<HTMLTextAreaElement>('body').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(2));
    // The title was changed, so the first Esc only arms the discard; the second leaves.
    const esc = (): void => void field<HTMLInputElement>('title').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    esc();
    expect(onCancel).not.toHaveBeenCalled();
    esc();
    expect(onCancel).toHaveBeenCalledTimes(1);
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
