import { describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import { readSettings, renderSettings, type SettingsActions } from './settings';

const actions: SettingsActions = {
  onSave: vi.fn(async () => undefined),
  onSetToken: vi.fn(async () => undefined),
  onClearToken: vi.fn(async () => undefined),
  onTest: vi.fn(async () => 'ok'),
  onClose: vi.fn(),
  onOpenLink: vi.fn(),
};

function render(): { form: HTMLFormElement; top: HTMLInputElement; bottom: HTMLInputElement } {
  const form = renderSettings({ settings: SAMPLE_SETTINGS, hasToken: false, actions }) as HTMLFormElement;
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
