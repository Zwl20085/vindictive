import { describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import { readSettings, renderSettings, type SettingsActions } from './settings';

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
