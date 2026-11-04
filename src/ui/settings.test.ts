import { describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import { normaliseShortcut, parseHighlightWords, readSettings, renderSettings, sameShortcut, type SettingsActions } from './settings';

const actions: SettingsActions = {
  onSave: vi.fn(async () => undefined),
  onPickFolder: vi.fn(async () => 'D:\\OneDrive\\Tips'),
  onOpenFolder: vi.fn(async () => undefined),
  onClose: vi.fn(),
};

function render(): { form: HTMLFormElement; top: HTMLInputElement; bottom: HTMLInputElement } {
  const form = renderSettings({ settings: SAMPLE_SETTINGS, actions }) as HTMLFormElement;
  document.body.append(form);
  const box = (name: string): HTMLInputElement => {
    const input = form.querySelector<HTMLInputElement>(`input[name="${name}"]`);
    if (!input) throw new Error(`missing ${name}`);
    return input;
  };
  return { form, top: box('always_on_top'), bottom: box('always_on_bottom') };
}

describe('window layer settings', () => {
  it('renders both boxes off by default and reads them back', () => {
    const { form, top, bottom } = render();
    expect(top.checked).toBe(false);
    expect(bottom.checked).toBe(false);
    bottom.checked = true;
    const read = readSettings(form, SAMPLE_SETTINGS);
    expect(read.always_on_bottom).toBe(true);
    expect(read.always_on_top).toBe(false);
    expect(SAMPLE_SETTINGS.always_on_bottom).toBe(false);
  });

  it('ticking one box clears the other', () => {
    const { top, bottom } = render();
    top.checked = true;
    top.dispatchEvent(new Event('change'));
    bottom.checked = true;
    bottom.dispatchEvent(new Event('change'));
    expect(top.checked).toBe(false);
    expect(bottom.checked).toBe(true);
    top.checked = true;
    top.dispatchEvent(new Event('change'));
    expect(bottom.checked).toBe(false);
  });

  it('rejects a form with both boxes on', () => {
    const { form, top, bottom } = render();
    top.checked = true;
    bottom.checked = true;
    expect(() => readSettings(form, SAMPLE_SETTINGS)).toThrow(/cannot both be on/);
  });
});

describe('tips folder setting', () => {
  it('reads the folder back and requires one', () => {
    const { form } = render();
    const input = form.querySelector<HTMLInputElement>('input[name="folder"]');
    if (!input) throw new Error('missing folder input');
    expect(input.value).toBe(SAMPLE_SETTINGS.folder);
    input.value = '  E:\\Sync\\Tips  ';
    expect(readSettings(form, SAMPLE_SETTINGS).folder).toBe('E:\\Sync\\Tips');
    input.value = '';
    expect(() => readSettings(form, SAMPLE_SETTINGS)).toThrow();
  });

  it('Browse fills the field with the picked folder', async () => {
    const { form } = render();
    const browse = [...form.querySelectorAll('button')].find((b) => b.textContent?.startsWith('Browse'));
    browse?.click();
    await vi.waitFor(() => expect(form.querySelector<HTMLInputElement>('input[name="folder"]')?.value).toBe('D:\\OneDrive\\Tips'));
  });
});

describe('hotkeys and updates', () => {
  it('reads the peek hotkey and the update check back', () => {
    const { form } = render();
    const peek = form.querySelector<HTMLInputElement>('input[name="board_hotkey"]');
    const updates = form.querySelector<HTMLInputElement>('input[name="auto_update_check"]');
    if (!peek || !updates) throw new Error('missing fields');
    expect(peek.value).toBe('Ctrl+Alt+Shift+Space');
    expect(updates.checked).toBe(true);
    peek.value = '  ';
    updates.checked = false;
    const read = readSettings(form, SAMPLE_SETTINGS);
    expect(read.board_hotkey).toBe('');
    expect(read.auto_update_check).toBe(false);
  });

  it('rejects the same shortcut for capture and peek', () => {
    const { form } = render();
    const peek = form.querySelector<HTMLInputElement>('input[name="board_hotkey"]');
    if (!peek) throw new Error('missing field');
    peek.value = 'ctrl+shift+SPACE';
    expect(() => readSettings(form, SAMPLE_SETTINGS)).toThrow(/different/);
    // Modifier order and spelling do not make it a different shortcut.
    peek.value = ' Shift + Control+space ';
    expect(() => readSettings(form, SAMPLE_SETTINGS)).toThrow(/different/);
  });

  it('requires a capture hotkey instead of silently keeping the old one', () => {
    const { form } = render();
    const hotkey = form.querySelector<HTMLInputElement>('input[name="hotkey"]');
    if (!hotkey) throw new Error('missing field');
    hotkey.value = '   ';
    expect(() => readSettings(form, SAMPLE_SETTINGS)).toThrow(/capture hotkey/);
  });

  it('shows the missing-hotkey error inline on save', async () => {
    const onSave = vi.fn(async () => undefined);
    const form = renderSettings({ settings: SAMPLE_SETTINGS, actions: { ...actions, onSave } }) as HTMLFormElement;
    document.body.append(form);
    const hotkey = form.querySelector<HTMLInputElement>('input[name="hotkey"]');
    if (!hotkey) throw new Error('missing field');
    hotkey.value = '';
    form.requestSubmit();
    await vi.waitFor(() => expect(form.querySelector('.set-status')?.textContent).toMatch(/capture hotkey/));
    expect(form.querySelector<HTMLElement>('.set-status')?.dataset.state).toBe('error');
    expect(onSave).not.toHaveBeenCalled();
  });
});

describe('shortcut comparison', () => {
  it('ignores modifier order, case, spacing and modifier spelling', () => {
    expect(normaliseShortcut('Ctrl+Shift+K')).toBe('ctrl+shift+k');
    expect(sameShortcut('Ctrl+Shift+K', 'Shift+Ctrl+K')).toBe(true);
    expect(sameShortcut(' ctrl + shift+k ', 'Ctrl+Shift+K')).toBe(true);
    expect(sameShortcut('Control+Alt+Space', 'Alt+Ctrl+Space')).toBe(true);
    expect(sameShortcut('Ctrl+Ctrl+K', 'Ctrl+K')).toBe(true);
    expect(sameShortcut('Ctrl+Shift+K', 'Ctrl+Shift+J')).toBe(false);
    expect(sameShortcut('Ctrl+Shift+K', 'Ctrl+K')).toBe(false);
    expect(sameShortcut('Ctrl+Alt+Shift+Space', 'Ctrl+Shift+Space')).toBe(false);
  });
});

describe('select labels', () => {
  it('translates known option values', () => {
    const { form } = render();
    const theme = form.querySelector<HTMLSelectElement>('select[name="theme"]');
    expect(Array.from(theme?.options ?? [], (o) => o.textContent)).toEqual(['dark', 'light', 'NERV', 'cobalt', 'paper']);
  });
});

describe('highlight words', () => {
  it('shows the words comma-separated with a hint and reads them back as a list', () => {
    const { form } = render();
    const input = form.querySelector<HTMLInputElement>('input[name="highlight_words"]');
    if (!input) throw new Error('missing highlight_words');
    expect(input.value).toBe(SAMPLE_SETTINGS.highlight_words.join(', '));
    expect(form.textContent).toContain('Shown with a mark on tiles and in notes');
    expect(readSettings(form, SAMPLE_SETTINGS).highlight_words).toEqual(SAMPLE_SETTINGS.highlight_words);
    input.value = ' Reviewer 2 , reviewer 2，截止日期, ';
    expect(readSettings(form, SAMPLE_SETTINGS).highlight_words).toEqual(['Reviewer 2', '截止日期']);
    input.value = '';
    expect(readSettings(form, SAMPLE_SETTINGS).highlight_words).toEqual([]);
  });
  it('rejects a word too short to highlight', () => {
    const { form } = render();
    const input = form.querySelector<HTMLInputElement>('input[name="highlight_words"]');
    if (input) input.value = 'a, ORCID';
    expect(() => readSettings(form, SAMPLE_SETTINGS)).toThrow(/too short/);
  });
  it('parses the list on its own', () => {
    expect(parseHighlightWords('one,two, two')).toEqual(['one', 'two']);
  });
});
