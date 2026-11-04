import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import type { BoardState, Tip } from '../types';

const backend = vi.hoisted(() => ({
  handlers: {} as Record<string, (args: unknown) => Promise<unknown>>,
  emit: undefined as ((board: unknown) => void) | undefined,
  listeners: new Map<string, (payload: unknown) => void>(),
}));

vi.mock('../api', () => ({
  isTauri: () => false,
  call: (cmd: string, args?: unknown) => (backend.handlers[cmd] ?? (async () => undefined))(args),
  onBoardUpdated: async (cb: (board: unknown) => void) => {
    backend.emit = cb;
    return () => undefined;
  },
  on: async (event: string, cb: (payload: unknown) => void) => {
    backend.listeners.set(event, cb);
    return () => undefined;
  },
  openExternal: vi.fn(async () => undefined),
}));

const { App, TICK_MS } = await import('./app');
const { isLightboxOpen, showLightbox } = await import('./lightbox');
const { closeMenu, isMenuOpen } = await import('./menu');

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
  closeMenu();
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

describe('App highlight words', () => {
  it('rebuilds an open detail when the highlight words change, not when they stay the same', async () => {
    await start();
    const panel = await openDetail('a');
    const withWords = (words: string[]): BoardState => {
      const b = board();
      return { ...b, settings: { ...b.settings, highlight_words: words } };
    };
    backend.emit?.(withWords(['coil']));
    const rebuilt = back().querySelector<HTMLElement>('.detail');
    expect(rebuilt).not.toBe(panel);
    backend.emit?.(withWords(['coil']));
    expect(back().querySelector('.detail')).toBe(rebuilt);
    backend.emit?.(withWords(['coil', 'ORCID']));
    expect(back().querySelector('.detail')).not.toBe(rebuilt);
  });
});

describe('App create form', () => {
  const openBar = (): HTMLInputElement => {
    (front().querySelector('.tile-add') as HTMLElement).click();
    const input = front().querySelector<HTMLInputElement>('.addbar-input');
    if (!input) throw new Error('no add bar');
    return input;
  };
  const createForm = (): HTMLFormElement | null => back().querySelector<HTMLFormElement>('.edit-form[data-editing="new"]');

  it('Ctrl+Enter in the add bar opens the full form prefilled from the capture line', async () => {
    await start();
    const input = openBar();
    input.value = 'Check coil #lab !high ^"Lab 302"';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }));
    const form = createForm();
    expect(form).not.toBeNull();
    expect(root.querySelector('.stage')?.classList.contains('open')).toBe(true);
    const value = (name: string): string => form?.querySelector<HTMLInputElement | HTMLSelectElement>(`[name="${name}"]`)?.value ?? '';
    expect(value('title')).toBe('Check coil');
    expect(value('tags')).toBe('lab');
    expect(value('priority')).toBe('high');
    expect(value('kind')).toBe('task');
    expect(value('location')).toBe('Lab 302');
    expect(form?.querySelector('button[type="submit"]')?.textContent).toBe('Create');
  });

  it('mounts the form once: the tick and board updates keep what is typed', async () => {
    await start();
    openBar().value = 'Draft';
    (front().querySelector('.addbar-more') as HTMLElement).click();
    const form = createForm();
    const body = form?.querySelector<HTMLTextAreaElement>('[name="body"]');
    if (body) body.value = 'typing';
    vi.advanceTimersByTime(TICK_MS);
    backend.emit?.(board([tip('a'), tip('b'), tip('c')]));
    expect(createForm()).toBe(form);
    expect(body?.value).toBe('typing');
  });

  it('saves through create_tip_full, closes the add bar and focuses the new tile', async () => {
    await start();
    const created = tip('new-one', { title: 'Brand new' });
    const createFull = vi.fn(async (_args: unknown) => board([tip('a'), tip('b'), created]));
    backend.handlers.create_tip_full = createFull;
    const quick = vi.fn(async () => board());
    backend.handlers.create_tip = quick;
    openBar().value = 'Brand new';
    (front().querySelector('.addbar-more') as HTMLElement).click();
    createForm()?.requestSubmit();
    await vi.waitFor(() => expect(root.querySelector('.stage')?.classList.contains('open')).toBe(false));
    expect(createFull).toHaveBeenCalledTimes(1);
    expect(quick).not.toHaveBeenCalled();
    expect(createFull.mock.calls[0]?.[0]).toMatchObject({ edit: { title: 'Brand new', kind: 'task', priority: 'normal', tags: [], remind: [] } });
    expect(front().querySelector('.addbar')).toBeNull();
    expect(document.activeElement).toBe(tile('new-one'));
  });

  it('keeps the form open with the error inline when creating fails', async () => {
    await start();
    backend.handlers.create_tip_full = async () => {
      throw new Error('folder is read-only');
    };
    openBar().value = 'Will fail';
    (front().querySelector('.addbar-more') as HTMLElement).click();
    const form = createForm();
    form?.requestSubmit();
    await vi.waitFor(() => expect(form?.querySelector('.set-status')?.textContent).toMatch(/read-only/));
    expect(createForm()).toBe(form);
    expect(root.querySelector('.stage')?.classList.contains('open')).toBe(true);
  });

  it('Esc on an untouched form returns to the board with the add bar text intact', async () => {
    await start();
    const input = openBar();
    input.value = 'Keep me';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }));
    createForm()?.querySelector('[name="title"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(root.querySelector('.stage')?.classList.contains('open')).toBe(false);
    expect(front().querySelector('.addbar-input')).toBe(input);
    expect(input.value).toBe('Keep me');
  });

  it('plain Enter in the add bar still quick-creates through create_tip', async () => {
    await start();
    const quick = vi.fn(async () => board());
    backend.handlers.create_tip = quick;
    const input = openBar();
    input.value = 'Quick one';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await vi.waitFor(() => expect(quick).toHaveBeenCalledWith({ text: 'Quick one' }));
    expect(createForm()).toBeNull();
  });
});

const stageOpen = (): boolean => !!root.querySelector('.stage')?.classList.contains('open');
const keydown = (target: EventTarget, key: string, extra: KeyboardEventInit = {}): KeyboardEvent => {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...extra });
  target.dispatchEvent(event);
  return event;
};
function openTileMenu(id: string): HTMLElement {
  tile(id).dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 20, clientY: 20 }));
  const menu = document.querySelector<HTMLElement>('.menu');
  if (!menu) throw new Error('no menu');
  return menu;
}
const menuItem = (menu: HTMLElement, label: string): HTMLElement =>
  Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]')).find((li) => li.textContent?.includes(label)) as HTMLElement;

describe('App with a context menu open', () => {
  it('board keys (n, arrows) do not act behind the menu', async () => {
    await start();
    const menu = openTileMenu('b');
    expect(isMenuOpen()).toBe(true);
    keydown(document.activeElement ?? document.body, 'n');
    keydown(document.activeElement ?? document.body, 'Insert');
    expect(front().querySelector('.addbar')).toBeNull();
    keydown(document.activeElement ?? document.body, 'ArrowDown');
    expect(menu.contains(document.activeElement)).toBe(true);
    keydown(document.activeElement ?? document.body, 'ArrowRight');
    expect(menu.contains(document.activeElement)).toBe(true);
    keydown(document.activeElement ?? document.body, 'Escape');
    expect(isMenuOpen()).toBe(false);
    keydown(document.body, 'n');
    expect(front().querySelector('.addbar')).not.toBeNull();
  });

  it('closes the tile menu when a board update replaces that tile', async () => {
    await start();
    openTileMenu('a');
    backend.emit?.(board([tip('a'), tip('b')]));
    expect(isMenuOpen()).toBe(true);
    backend.emit?.(board([tip('a', { sha: 'sha-a2' }), tip('b')]));
    expect(isMenuOpen()).toBe(false);
  });
});

describe('App edit origin', () => {
  it('Edit from the tile menu returns to the board on Cancel, with the tile focused', async () => {
    await start();
    menuItem(openTileMenu('b'), 'Edit').click();
    const form = back().querySelector<HTMLFormElement>('.edit-form[data-editing="b"]');
    expect(form).not.toBeNull();
    keydown(form?.querySelector('[name="title"]') as HTMLElement, 'Escape');
    expect(stageOpen()).toBe(false);
    expect(back().querySelector('.detail')).toBeNull();
    expect(document.activeElement).toBe(tile('b'));
  });

  it('Edit from the tile menu returns to the board after saving', async () => {
    await start();
    backend.handlers.update_tip = async () => board([tip('a'), tip('b', { sha: 'sha-b2', title: 'Renamed' })]);
    menuItem(openTileMenu('b'), 'Edit').click();
    back().querySelector<HTMLFormElement>('.edit-form')?.requestSubmit();
    await vi.waitFor(() => expect(stageOpen()).toBe(false));
    expect(back().querySelector('.detail')).toBeNull();
  });

  it('Edit from the detail still returns to the detail', async () => {
    await start();
    const panel = await openDetail('a');
    (Array.from(panel.querySelectorAll<HTMLButtonElement>('.action')).find((b) => b.textContent === 'Edit') as HTMLButtonElement).click();
    const form = back().querySelector<HTMLFormElement>('.edit-form[data-editing="a"]');
    expect(form).not.toBeNull();
    keydown(form?.querySelector('[name="title"]') as HTMLElement, 'Escape');
    expect(stageOpen()).toBe(true);
    expect(back().querySelector('.detail')).not.toBeNull();
  });
});

describe('App settings Esc', () => {
  it('asks before discarding changed settings; a second Esc closes', async () => {
    const holder = document.createElement('div');
    document.body.append(holder);
    root = holder;
    backend.handlers.get_state = async () => board();
    const app = new App(holder);
    await app.start();
    app.openSettings();
    const form = holder.querySelector<HTMLFormElement>('.face-back .settings') as HTMLFormElement;
    // Untouched: one Esc closes.
    keydown(document.body, 'Escape');
    expect(stageOpen()).toBe(false);
    app.openSettings();
    const again = holder.querySelector<HTMLFormElement>('.face-back .settings') as HTMLFormElement;
    expect(again).not.toBe(form);
    (again.querySelector('input[name="weather_location"]') as HTMLInputElement).value = 'Kyoto';
    keydown(document.body, 'Escape');
    expect(stageOpen()).toBe(true);
    expect(again.querySelector('.set-status')?.textContent).toMatch(/Esc again/);
    keydown(document.body, 'Escape');
    expect(stageOpen()).toBe(false);
  });
});

describe('App when the shown tip disappears', () => {
  it('closes the lightbox and returns to the board from the detail', async () => {
    await start();
    await openDetail('a');
    showLightbox('data:image/png;base64,', 'figure');
    expect(isLightboxOpen()).toBe(true);
    backend.emit?.(board([tip('b')]));
    expect(isLightboxOpen()).toBe(false);
    expect(stageOpen()).toBe(false);
  });

  it('keeps a dirty edit form open with a notice; a clean one returns to the board', async () => {
    await start();
    menuItem(openTileMenu('a'), 'Edit').click();
    const form = back().querySelector<HTMLFormElement>('.edit-form[data-editing="a"]') as HTMLFormElement;
    (form.querySelector('[name="body"]') as HTMLTextAreaElement).value = 'unsaved words';
    backend.emit?.(board([tip('b')]));
    expect(stageOpen()).toBe(true);
    expect(back().querySelector('.edit-form')).toBe(form);
    expect((form.querySelector('[name="body"]') as HTMLTextAreaElement).value).toBe('unsaved words');
    expect(form.querySelector('.set-status')?.textContent).toMatch(/removed or renamed/);
    // Cancel cannot go back to a detail that no longer exists: the board.
    form.querySelector<HTMLButtonElement>('.action-back')?.click();
    expect(stageOpen()).toBe(false);

    backend.emit?.(board([tip('a'), tip('b')]));
    menuItem(openTileMenu('a'), 'Edit').click();
    expect(back().querySelector('.edit-form[data-editing="a"]')).not.toBeNull();
    backend.emit?.(board([tip('b')]));
    expect(stageOpen()).toBe(false);
  });
});

describe('App chrome strip', () => {
  it('updates the strip in place, keeping focus on the sync square; a language switch rebuilds it', async () => {
    await start();
    const strip = root.querySelector<HTMLElement>('.chrome');
    const dot = strip?.querySelector<HTMLButtonElement>('.sync-dot') as HTMLButtonElement;
    dot.focus();
    let release: (b: BoardState) => void = () => undefined;
    backend.handlers.sync_now = () => new Promise<BoardState>((resolve) => (release = resolve));
    dot.click();
    expect(dot.dataset.state).toBe('syncing');
    release(board());
    await vi.waitFor(() => expect(dot.dataset.state).toBe('ok'));
    expect(root.querySelector('.chrome')).toBe(strip);
    expect(document.activeElement).toBe(dot);
    backend.emit?.({ ...board(), settings: { ...board().settings, language: 'zh' } });
    expect(root.querySelector('.chrome')).not.toBe(strip);
  });
});

describe('App fit height', () => {
  const heightDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
  const innerHeight = Object.getOwnPropertyDescriptor(window, 'innerHeight');
  const setWindowHeight = (h: number): void => {
    Object.defineProperty(window, 'innerHeight', { configurable: true, writable: true, value: h });
  };
  beforeEach(() => {
    // jsdom has no layout: the grid is 100 px per tile, everything else 0.
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
      configurable: true,
      get(this: HTMLElement) {
        return this.classList.contains('board') ? 100 * this.querySelectorAll('.tile[data-id]').length : 0;
      },
    });
    setWindowHeight(768);
  });
  afterEach(() => {
    if (heightDescriptor) Object.defineProperty(HTMLElement.prototype, 'offsetHeight', heightDescriptor);
    if (innerHeight) Object.defineProperty(window, 'innerHeight', innerHeight);
  });
  const fitBoard = (tips: Tip[]): BoardState => {
    const b = board(tips);
    return { ...b, settings: { ...b.settings, fit_height: true } };
  };

  it('leaves a height the user chose until the board layout changes', async () => {
    const fit = vi.fn(async (_args: unknown) => undefined);
    backend.handlers.fit_window = fit;
    await start(fitBoard([tip('a'), tip('b')]));
    expect(fit).toHaveBeenLastCalledWith({ height: 200 });
    // The window answers the request.
    setWindowHeight(200);
    window.dispatchEvent(new Event('resize'));
    // The user drags the bottom edge.
    setWindowHeight(500);
    window.dispatchEvent(new Event('resize'));
    const calls = fit.mock.calls.length;
    vi.advanceTimersByTime(TICK_MS * 3);
    backend.emit?.(fitBoard([tip('a', { sha: 'sha-a2' }), tip('b')]));
    expect(fit.mock.calls.length).toBe(calls);
    // One more tile: the layout changed, fit again.
    backend.emit?.(fitBoard([tip('a'), tip('b'), tip('c')]));
    expect(fit).toHaveBeenLastCalledWith({ height: 300 });
  });

  const resizeTo = (h: number): void => {
    setWindowHeight(h);
    window.dispatchEvent(new Event('resize'));
  };
  const announce = (height: number): void => backend.listeners.get('app-sized')?.({ width: 400, height });

  it('a resize the backend announced (app-sized) does not suspend fitting', async () => {
    const fit = vi.fn(async (_args: unknown) => undefined);
    backend.handlers.fit_window = fit;
    await start(fitBoard([tip('a'), tip('b')]));
    announce(200);
    resizeTo(200);
    const calls = fit.mock.calls.length;
    // A dock or clamp: announced, then applied. Not the user's, so fit again.
    announce(640);
    resizeTo(640);
    expect(fit.mock.calls.length).toBe(calls + 1);
    expect(fit).toHaveBeenLastCalledWith({ height: 200 });
  });

  it('an unannounced resize after sizing has started is the user\'s', async () => {
    const fit = vi.fn(async (_args: unknown) => undefined);
    backend.handlers.fit_window = fit;
    await start(fitBoard([tip('a'), tip('b')]));
    announce(200);
    resizeTo(200);
    announce(640); // announced, but a different height
    resizeTo(480);
    const calls = fit.mock.calls.length;
    vi.advanceTimersByTime(TICK_MS);
    backend.emit?.(fitBoard([tip('a', { sha: 'sha-a2' }), tip('b')]));
    expect(fit.mock.calls.length).toBe(calls);
  });

  it('resizes before any sizing round-trip (startup) are not the user\'s', async () => {
    const fit = vi.fn(async (_args: unknown) => undefined);
    backend.handlers.fit_window = fit;
    const off = board([tip('a'), tip('b')]); // fit_height off: no fit_window round-trip yet
    await start(off);
    resizeTo(900);
    backend.emit?.(fitBoard([tip('a'), tip('b')]));
    expect(fit).toHaveBeenLastCalledWith({ height: 200 });
  });

  it('a width-only resize is not taken for a manual height', async () => {
    const fit = vi.fn(async (_args: unknown) => undefined);
    backend.handlers.fit_window = fit;
    await start(fitBoard([tip('a'), tip('b')]));
    setWindowHeight(200);
    window.dispatchEvent(new Event('resize'));
    window.dispatchEvent(new Event('resize'));
    backend.emit?.(fitBoard([tip('a'), tip('b'), tip('c')]));
    expect(fit).toHaveBeenLastCalledWith({ height: 300 });
    // Shrinking back to two tiles fits again, too.
    setWindowHeight(300);
    window.dispatchEvent(new Event('resize'));
    backend.emit?.(fitBoard([tip('a'), tip('b')]));
    expect(fit).toHaveBeenLastCalledWith({ height: 200 });
  });
});
