import { afterEach, describe, expect, it, vi } from 'vitest';

const call = vi.fn();
vi.mock('../api', () => ({
  call: (...args: unknown[]) => call(...args),
  on: vi.fn(async () => () => undefined),
}));

const { HINT_RESET_MS, renderCapture } = await import('./capture');

afterEach(() => {
  vi.useRealTimers();
  call.mockReset();
  document.body.replaceChildren();
});

function setup(): { input: HTMLInputElement; hint: HTMLElement } {
  const root = document.createElement('div');
  document.body.append(root);
  renderCapture(root);
  const input = root.querySelector<HTMLInputElement>('.capture-input');
  const hint = root.querySelector<HTMLElement>('.capture-hint');
  if (!input || !hint) throw new Error('capture not rendered');
  return { input, hint };
}

const enter = (input: HTMLInputElement): void => void input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));

describe('capture hint', () => {
  it('a second message gets its full time; the first one\'s reset is cancelled', async () => {
    vi.useFakeTimers();
    call.mockImplementation(async (cmd: string) => {
      if (cmd === 'create_tip') throw new Error('create_tip: disk full');
      if (cmd === 'get_state') throw new Error('offline');
      return undefined;
    });
    const { input, hint } = setup();
    expect(input.getAttribute('aria-label')).toBe('New tip');
    input.value = 'first';
    enter(input);
    await vi.waitFor(() => expect(hint.dataset.state).toBe('error'));
    vi.advanceTimersByTime(HINT_RESET_MS - 1000);
    input.value = 'second';
    enter(input);
    await vi.waitFor(() => expect(call).toHaveBeenCalledTimes(3)); // get_state + two creates
    expect(hint.textContent).toBe('create_tip: disk full');
    // The first message's timer would fire here; it must not wipe the second.
    vi.advanceTimersByTime(1500);
    expect(hint.dataset.state).toBe('error');
    vi.advanceTimersByTime(HINT_RESET_MS);
    expect(hint.dataset.state).toBe('');
  });

  it('typing cancels a pending reset and shows the live preview', async () => {
    vi.useFakeTimers();
    call.mockImplementation(async (cmd: string) => {
      if (cmd === 'create_tip') throw new Error('nope');
      if (cmd === 'get_state') throw new Error('offline');
      return undefined;
    });
    const { input, hint } = setup();
    input.value = 'x';
    enter(input);
    await vi.waitFor(() => expect(hint.dataset.state).toBe('error'));
    input.value = 'Check coil !high';
    input.dispatchEvent(new Event('input'));
    vi.advanceTimersByTime(HINT_RESET_MS + 10);
    expect(hint.textContent).toBe('Check coil · high');
    expect(hint.dataset.state).toBe('');
  });
});
