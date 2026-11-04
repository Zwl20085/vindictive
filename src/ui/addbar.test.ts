import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '../lib/i18n';
import { renderAddBar, type AddBarActions } from './addbar';

afterEach(() => {
  setLocale('en');
  document.body.innerHTML = '';
});

function setup(onSubmit: AddBarActions['onSubmit'] = vi.fn(async () => undefined)) {
  const actions = { onSubmit, onExpand: vi.fn(), onCancel: vi.fn() };
  const bar = renderAddBar(actions);
  document.body.append(bar);
  const input = bar.querySelector<HTMLInputElement>('.addbar-input');
  if (!input) throw new Error('no input');
  const key = (init: KeyboardEventInit): void => void input.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, ...init }));
  return { bar, input, actions, key };
}

describe('renderAddBar', () => {
  it('Enter quick-creates the trimmed text', async () => {
    const { input, actions, key } = setup();
    input.value = '  Check coil #lab ';
    key({ key: 'Enter' });
    await vi.waitFor(() => expect(actions.onSubmit).toHaveBeenCalledWith('Check coil #lab'));
    expect(actions.onExpand).not.toHaveBeenCalled();
  });

  it('Ctrl+Enter and the More button expand to the full form with the current text', () => {
    const { bar, input, actions, key } = setup();
    input.value = 'Check coil';
    key({ key: 'Enter', ctrlKey: true });
    expect(actions.onExpand).toHaveBeenCalledWith('Check coil');
    expect(actions.onSubmit).not.toHaveBeenCalled();
    input.value = '';
    bar.querySelector<HTMLButtonElement>('.addbar-more')?.click();
    expect(actions.onExpand).toHaveBeenLastCalledWith('');
  });

  it('Esc cancels without reaching the board', () => {
    const { actions, key } = setup();
    const bubble = vi.fn();
    document.addEventListener('keydown', bubble);
    key({ key: 'Escape' });
    document.removeEventListener('keydown', bubble);
    expect(actions.onCancel).toHaveBeenCalledTimes(1);
    expect(bubble).not.toHaveBeenCalled();
  });

  it('labels the More button in the UI language', () => {
    setLocale('zh');
    const { bar } = setup();
    expect(bar.querySelector('.addbar-more')?.textContent).toBe('更多…');
  });
});
