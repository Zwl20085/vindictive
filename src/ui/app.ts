import { call, onBoardUpdated, openExternal } from '../api';
import type { BoardState, Dock, Settings } from '../types';
import { handleBoardKeys, renderBoard } from './board';
import { renderChrome } from './chrome';
import { renderDetail } from './detail';
import { el, mount } from './dom';
import { errorLine, guard, showError } from './errors';
import { renderSettings } from './settings';
import { Store, type UiState } from './store';

/** Re-render countdowns this often. */
export const TICK_MS = 30_000;

export class App {
  private readonly store = new Store();
  private readonly stage = el('main', { className: 'stage' });
  private readonly front = el('div', { className: 'face face-front' });
  private readonly back = el('div', { className: 'face face-back' });

  constructor(private readonly root: HTMLElement) {}

  async start(): Promise<void> {
    this.stage.append(this.front, this.back);
    mount(this.root, el('div', { className: 'chrome-slot' }), errorLine(), this.stage);
    this.store.subscribe(() => this.render());
    document.addEventListener('keydown', (e) => this.onKey(e));
    await onBoardUpdated((board) => this.applyBoard(board)).catch((e) => showError('event subscription', e));
    await this.refresh('get_state');
    this.openFromHash();
    setInterval(() => this.render(), TICK_MS);
  }

  /** `#tip=<id>` opens that tip's detail on load (dev and deep links). */
  private openFromHash(): void {
    const match = /^#tip=(.+)$/.exec(window.location.hash);
    const id = match?.[1] ? decodeURIComponent(match[1]) : undefined;
    if (id && this.store.get().board?.tips.some((t) => t.id === id)) {
      this.store.set({ view: { kind: 'detail', id } });
    }
  }

  private applyBoard(board: BoardState): void {
    document.documentElement.dataset.theme = board.settings.theme;
    const view = this.store.get().view;
    const stillExists = view.kind !== 'detail' || board.tips.some((t) => t.id === view.id);
    this.store.set({ board, sync: board.sync_error ? 'error' : 'ok', view: stillExists ? view : { kind: 'board' } });
  }

  private async refresh(cmd: 'get_state' | 'sync_now'): Promise<void> {
    this.store.set({ sync: 'syncing' });
    const board = await guard(cmd, () => call(cmd));
    if (board) this.applyBoard(board);
    else this.store.set({ sync: 'error' });
  }

  private async mutate(action: () => Promise<BoardState>, context: string): Promise<void> {
    const board = await guard(context, action);
    if (board) this.applyBoard(board);
  }

  private onKey(event: KeyboardEvent): void {
    const { view } = this.store.get();
    if (event.key === 'Escape' && view.kind !== 'board') {
      this.store.set({ view: { kind: 'board' } });
      return;
    }
    if (view.kind === 'board') {
      const grid = this.front.querySelector<HTMLElement>('.board');
      if (grid) handleBoardKeys(grid, event);
    }
  }

  private render(): void {
    const state = this.store.get();
    const slot = this.root.querySelector('.chrome-slot');
    if (slot) mount(slot, renderChrome({ state: state.board, sync: state.sync, actions: this.chromeActions() }));
    if (!state.board) return;
    const now = new Date();
    mount(this.front, renderBoard({ state: state.board, now, onOpen: (id) => this.store.set({ view: { kind: 'detail', id } }) }));
    this.renderBack(state, now);
    this.stage.classList.toggle('open', state.view.kind !== 'board');
  }

  private renderBack(state: UiState, now: Date): void {
    const board = state.board;
    if (!board) return;
    if (state.view.kind === 'settings') {
      mount(this.back, renderSettings({ settings: board.settings, hasToken: board.has_token, actions: this.settingsActions() }));
      return;
    }
    if (state.view.kind === 'detail') {
      const id = state.view.id;
      const tip = board.tips.find((t) => t.id === id);
      if (tip) mount(this.back, renderDetail({ tip, settings: board.settings, now, actions: this.detailActions() }));
      return;
    }
  }

  private back_(): void {
    this.store.set({ view: { kind: 'board' } });
  }

  private chromeActions() {
    return {
      onSync: () => void this.refresh('sync_now'),
      onSettings: () => this.store.set({ view: { kind: 'settings' } }),
      onToggleDone: () => {
        const s = this.store.get().board?.settings;
        if (s) void this.saveSettings({ ...s, show_done: !s.show_done });
      },
      onDock: (dock: Dock) => {
        // save_settings applies the dock on the backend: one call, one emit.
        const s = this.store.get().board?.settings;
        if (s) void this.saveSettings({ ...s, dock });
        else void guard('dock_window', () => call('dock_window', { dock }));
      },
      onQuit: () => void guard('quit', () => call('quit')),
    };
  }

  private async saveSettings(settings: Settings): Promise<void> {
    await this.mutate(() => call('save_settings', { settings }), 'save_settings');
  }

  private detailActions() {
    return {
      onDone: (id: string) => void this.mutate(() => call('mark_done', { id }), 'mark_done').then(() => this.back_()),
      onReopen: (id: string) => void this.mutate(() => call('reopen', { id }), 'reopen'),
      onSnooze: (id: string, minutes: number) => void this.mutate(() => call('snooze', { id, minutes }), 'snooze').then(() => this.back_()),
      onOpenLink: (url: string) => void guard('open link', () => openExternal(url)),
      onBack: () => this.back_(),
      resolveImage: (path: string) => call('fetch_image', { path }),
    };
  }

  private settingsActions() {
    return {
      onSave: async (settings: Settings) => {
        const board = await call('save_settings', { settings });
        this.applyBoard(board);
      },
      onSetToken: async (token: string) => {
        await call('set_token', { token });
        await this.refresh('sync_now');
      },
      onClearToken: async () => {
        await call('clear_token');
        await this.refresh('get_state');
      },
      onTest: () => call('test_connection'),
      onClose: () => this.back_(),
    };
  }

  openSettings(): void {
    this.store.set({ view: { kind: 'settings' } });
  }
}
