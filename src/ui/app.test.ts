import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import type { BoardState, Tip } from '../types';

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: unknown) => Promise<unknown>>,
  emit: undefined as ((board: unknown) => void) | undefined,
}));

vi.mock('../api', () => ({
  isTauri: () => false,
  call: (cmd: string, args?: unknown) => (backend.handlers[cmd] ?? (async () => undefined))(args),
  onBoardUpdated: async (cb: (board: unknown) => void) => {
    backend.emit = cb;
    return () => undefined;
  },
  on: async () => () => undefined,
  openExternal: vi.fn(async () => undefined),
}));

const { App, TICK_MS } = await import('./app');

function tip(id: string, extra: Partial<Tip> = {}): Tip {
  return { id, path: `${id}.md`, sha: `sha-${id}`, title: `Tip ${id}`, kind: 'task', priority: 'normal', status: 'open', body: 'Body', remind_at: [], ...extra };
}

function board(tips: Tip[] = [tip('a'), tip('b')]): BoardState {
  return {
    tips,
    settings: { ...SAMPLE_SETTINGS, weather_location: '', fit_height: false, show_panel: false, language: 'en' },
    now: '2026-11-04T10:00:00',
  };
}

let root: HTMLElement;

async function start(initial = board()): Promise<void> {
  backend.handlers.get_state = async () => initial;
  root = document.createElement('div');
  document.body.append(root);
  await new App(root).start();
}

const front = (): HTMLElement => root.querySelector<HTMLElement>('.face-front') as HTMLElement;
const back = (): HTMLElement => root.querySelector<HTMLElement>('.face-back') as HTMLElement;
const tile = (id: string): HTMLElement =>
  Array.from(front().querySelectorAll<HTMLElement>('.tile[data-id]')).find((t) => t.dataset.id === id) as HTMLElement;

async function openDetail(id: string): Promise<HTMLElement> {
  tile(id).click();
  await vi.waitFor(() => expect(back().querySelector('.detail')).not.toBeNull());
  return back().querySelector('.detail') as HTMLElement;
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
  // Reduced motion: every `animate()` resolves at once.
  window.matchMedia = ((query: string) => ({
    matches: query.includes('reduce'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    onchange: null,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
  backend.handlers = { frontend_log: async () => undefined };
  window.location.hash = '';
});

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
  document.body.innerHTML = '';
});

describe('App rendering', () => {
  it('keeps the add bar and its text across the tick and board updates', async () => {
    await start();
    (front().querySelector('.tile-add') as HTMLElement).click();
    const input = front().querySelector<HTMLInputElement>('.addbar-input');
    expect(input).not.toBeNull();
    if (!input) return;
    input.value = 'half typed';
    vi.advanceTimersByTime(TICK_MS);
    backend.emit?.(board([tip('a'), tip('b'), tip('c')]));
    expect(front().querySelector('.addbar-input')).toBe(input);
    expect(input.value).toBe('half typed');
    expect(front().firstElementChild?.classList.contains('addbar')).toBe(true);
  });

  it('mounts the settings form once and keeps what is typed', async () => {
    const holder = document.createElement('div');
    document.body.append(holder);
    backend.handlers.get_state = async () => board();
    const app = new App(holder);
    await app.start();
    app.openSettings();
    const form = holder.querySelector<HTMLFormElement>('.face-back .settings');
    expect(form).not.toBeNull();
    const input = form?.querySelector<HTMLInputElement>('input[name="weather_location"]');
    expect(input).not.toBeNull();
    if (input) input.value = 'typing';
    vi.advanceTimersByTime(TICK_MS);
    backend.emit?.(board([tip('a')]));
    expect(holder.querySelector('.face-back .settings')).toBe(form);
    expect(input?.value).toBe('typing');
  });

  it('re-opening settings renders the current settings', async () => {
    const holder = document.createElement('div');
    document.body.append(holder);
    backend.handlers.get_state = async () => board();
    const app = new App(holder);
    await app.start();
    app.openSettings();
    const first = holder.querySelector('.face-back .settings');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(holder.querySelector('.stage')?.classList.contains('open')).toBe(false);
    backend.emit?.({ ...board(), settings: { ...board().settings, weather_location: 'Kyoto' } });
    app.openSettings();
    const second = holder.querySelector('.face-back .settings');
    expect(second).not.toBe(first);
    expect(second?.querySelector<HTMLInputElement>('input[name="weather_location"]')?.value).toBe('Kyoto');
  });

  it('toggles inert on the face that is turned away', async () => {
    await start();
    expect(front().hasAttribute('inert')).toBe(false);
    expect(back().hasAttribute('inert')).toBe(true);
    await openDetail('a');
    expect(front().hasAttribute('inert')).toBe(true);
    expect(back().hasAttribute('inert')).toBe(false);
  });

  it('keeps the detail panel when the shown tip is unchanged, rebuilds it on a new sha', async () => {
    await start();
    const panel = await openDetail('a');
    backend.emit?.(board([tip('a'), tip('b'), tip('c')]));
    vi.advanceTimersByTime(TICK_MS);
    expect(back().querySelector('.detail')).toBe(panel);
    backend.emit?.(board([tip('a', { sha: 'sha-a2', title: 'Edited' }), tip('b')]));
    const fresh = back().querySelector<HTMLElement>('.detail');
    expect(fresh).not.toBe(panel);
    expect(fresh?.dataset.sha).toBe('sha-a2');
  });

  it('focuses the detail when it opens and the tile again on Back', async () => {
    await start();
    tile('b').focus();
    const panel = await openDetail('b');
    expect(document.activeElement).toBe(panel);
    (panel.querySelector('.action-back') as HTMLElement).click();
    expect(root.querySelector('.stage')?.classList.contains('open')).toBe(false);
    expect(document.activeElement).toBe(tile('b'));
  });

  it('keeps tile nodes and focus across the tick', async () => {
    await start();
    const a = tile('a');
    a.focus();
    vi.advanceTimersByTime(TICK_MS);
    expect(tile('a')).toBe(a);
    expect(document.activeElement).toBe(a);
  });

  it('stays on the detail when delete fails, with its actions usable again', async () => {
    await start();
    backend.handlers.delete_tip = async () => {
      throw new Error('disk full');
    };
    const panel = await openDetail('a');
    const del = Array.from(panel.querySelectorAll<HTMLButtonElement>('.action')).find((b) => b.textContent === 'Delete') as HTMLButtonElement;
    del.click();
    del.click();
    await vi.waitFor(() => expect(back().querySelector('.detail')).not.toBe(panel));
    expect(root.querySelector('.stage')?.classList.contains('open')).toBe(true);
    const actions = Array.from(back().querySelectorAll<HTMLButtonElement>('.detail .action'));
    expect(actions.length).toBeGreaterThan(0);
    expect(actions.every((b) => !b.disabled)).toBe(true);
  });

  it('goes back after a successful done', async () => {
    await start();
    backend.handlers.mark_done = async () => board([tip('a', { status: 'done', sha: 'sha-a-done' }), tip('b')]);
    const panel = await openDetail('a');
    (panel.querySelector('.action-primary') as HTMLElement).click();
    await vi.waitFor(() => expect(root.querySelector('.stage')?.classList.contains('open')).toBe(false));
  });
});
