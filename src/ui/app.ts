import { call, isTauri, onBoardUpdated, openExternal } from '../api';
import type { TileSize } from '../lib/tileSize';
import type { BoardState, Dock, Settings, Tip, TipEdit } from '../types';
import { setLocale, t } from '../lib/i18n';
import { devClawdMood } from '../mock/sample-tips';
import { renderAddBar } from './addbar';
import { showMenu } from './menu';
import { fitTitles, handleBoardKeys, renderBoard } from './board';
import { renderChrome } from './chrome';
import { renderDetail } from './detail';
import { el, mount } from './dom';
import { renderEditForm } from './edit-form';
import { errorLine, guard, showError, showNotice } from './errors';
import { cachedResolver } from './images';
import { closeLightbox, isLightboxOpen } from './lightbox';
import { Panel } from './panel';
import { renderSettings } from './settings';
import { Store, type UiState } from './store';
import { tileMenuItems } from './tile-menu';

/** First-load retries while the backend finishes starting. */
export const STARTUP_ATTEMPTS = 20;
export const STARTUP_RETRY_MS = 150;

/** Re-render countdowns this often. */
export const TICK_MS = 30_000;
/** Ask the backend for fresh weather this often (it caches on its own too). */
export const WEATHER_MS = 20 * 60_000;

export class App {
  private readonly store = new Store();
  private readonly stage = el('main', { className: 'stage' });
  private readonly front = el('div', { className: 'face face-front' });
  private readonly back = el('div', { className: 'face face-back' });
  private readonly panel = new Panel();
  /** Tile ids that have already played their entry animation. */
  private readonly seen = new Set<string>();
  private readonly resolveImage = cachedResolver((path) => call('fetch_image', { path }));
  private weatherFor = '';

  constructor(private readonly root: HTMLElement) {}

  async start(): Promise<void> {
    this.stage.append(this.front, this.back);
    mount(this.root, el('div', { className: 'chrome-slot' }), errorLine(), this.panel.element, this.stage);
    this.panel.start();
    if (!isTauri()) this.panel.forceClawd(devClawdMood());
    // Chrome and panel are cheap and update on every change; the tile faces
    // are rebuilt only when the board or the view changes (and on the tick),
    // so weather / sync updates never interrupt a running tile animation.
    this.store.subscribe((state, previous) => {
      this.renderChrome(state);
      this.renderPanel(state);
      if (state.board !== previous.board || state.view !== previous.view || state.adding !== previous.adding) this.renderFaces(state);
    });
    document.addEventListener('keydown', (e) => this.onKey(e));
    await onBoardUpdated((board) => this.applyBoard(board)).catch((e) => showError('event subscription', e));
    await this.loadInitialState();
    this.openFromHash();
    setInterval(() => this.renderFaces(this.store.get()), TICK_MS);
    setInterval(() => void this.loadWeather(true), WEATHER_MS);
    window.addEventListener('resize', () => this.scheduleFit());
    window.addEventListener('resize', () => this.userResized());
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
    document.documentElement.style.setProperty('--alpha', String(Math.min(100, Math.max(20, board.settings.window_opacity || 100)) / 100));
    setLocale(board.settings.language);
    const view = this.store.get().view;
    const stillExists = !('id' in view) || board.tips.some((t) => t.id === view.id);
    this.store.set({ board, sync: board.sync_error ? 'error' : 'ok', view: stillExists ? view : { kind: 'board' } });
    void this.loadWeather(false);
  }

  /** Fetch weather when the configured place changed, or when `force` (periodic refresh). */
  private async loadWeather(force: boolean): Promise<void> {
    const settings = this.store.get().board?.settings;
    const place = settings?.weather_location.trim() ?? '';
    if (!place) {
      this.weatherFor = '';
      if (this.store.get().weatherStatus !== 'off') this.store.set({ weather: undefined, weatherStatus: 'off' });
      return;
    }
    if (!force && place === this.weatherFor) return;
    this.weatherFor = place;
    if (!this.store.get().weather) this.store.set({ weatherStatus: 'loading' });
    try {
      const weather = await call('fetch_weather');
      if (this.weatherFor !== place) return; // settings changed while waiting
      this.store.set(weather ? { weather, weatherStatus: 'ok' } : { weather: undefined, weatherStatus: 'off' });
    } catch (error) {
      console.warn('Vindictive: weather unavailable', error);
      void call('frontend_log', { level: 'warn', message: `weather: ${error instanceof Error ? error.message : String(error)}` }).catch(() => undefined);
      this.store.set({ weatherStatus: 'error' });
    }
  }

  /**
   * First load. The window can start before the backend has registered its
   * state, so early failures are retried quietly; only a lasting failure is
   * shown. The backend also emits the board after its first folder scan.
   */
  private async loadInitialState(): Promise<void> {
    for (let attempt = 1; attempt <= STARTUP_ATTEMPTS; attempt++) {
      try {
        this.applyBoard(await call('get_state'));
        return;
      } catch (error) {
        if (this.store.get().board) return; // the backend's emit got here first
        if (attempt === STARTUP_ATTEMPTS) {
          showError('get_state', error);
          this.store.set({ sync: 'error' });
          return;
        }
        await new Promise((resolve) => setTimeout(resolve, STARTUP_RETRY_MS));
      }
    }
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
    if (isLightboxOpen()) return; // the lightbox handles Esc itself
    const { view, adding } = this.store.get();
    if (view.kind === 'edit') return; // the edit form handles Esc and Ctrl+Enter
    const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
    if (event.key === 'Escape' && view.kind !== 'board') {
      this.back_();
      return;
    }
    if (view.kind === 'board' && !typing) {
      if ((event.key === 'n' || event.key === '+' || event.key === 'Insert') && !adding) {
        event.preventDefault();
        this.store.set({ adding: true });
        return;
      }
      const grid = this.front.querySelector<HTMLElement>('.board');
      if (grid) handleBoardKeys(grid, event);
    }
  }

  /** Right-click menu on a tile: the detail actions without flipping. */
  private tileMenu(id: string, x: number, y: number): void {
    const tip = this.store.get().board?.tips.find((t) => t.id === id);
    if (tip) showMenu(x, y, tileMenuItems(tip, this.detailActions(), x, y));
  }

  private renderChrome(state: UiState): void {
    const slot = this.root.querySelector('.chrome-slot');
    if (slot) mount(slot, renderChrome({ state: state.board, sync: state.sync, actions: this.chromeActions() }));
  }

  private renderPanel(state: UiState): void {
    this.panel.update({
      visible: state.board?.settings.show_panel ?? true,
      language: state.board?.settings.language ?? 'en',
      weather: state.weather,
      weatherStatus: state.weatherStatus,
      tips: state.board?.tips,
    });
  }

  private renderFaces(state: UiState): void {
    if (!state.board) return;
    const now = new Date();
    mount(
      this.front,
      state.adding ? renderAddBar(this.addBarActions()) : null,
      renderBoard({
        state: state.board,
        now,
        seen: this.seen,
        resolveImage: this.resolveImage,
        onOpen: (id) => this.store.set({ view: { kind: 'detail', id } }),
        onMenu: (id, x, y) => this.tileMenu(id, x, y),
        onAdd: () => this.store.set({ adding: true }),
        onSetup: () => this.store.set({ view: { kind: 'settings' } }),
        onReorder: (id, order) => void this.mutate(() => call('set_order', { id, order }), 'set_order'),
      }),
    );
    fitTitles(this.front);
    this.renderBack(state, now);
    this.stage.classList.toggle('open', state.view.kind !== 'board');
    this.fitWindow();
  }

  private fitTimer: ReturnType<typeof setTimeout> | undefined;
  private lastRequestedHeight = 0;

  /** Ask the backend to size the window to the board (setting `fit_height`). */
  private fitWindow(): void {
    const state = this.store.get();
    if (!state.board?.settings.fit_height || state.view.kind !== 'board') return;
    const grid = this.front.querySelector<HTMLElement>('.board');
    if (!grid) return;
    const h = (el: Element | null | undefined): number => (el instanceof HTMLElement && !el.hidden ? el.offsetHeight : 0);
    const height = Math.ceil(h(this.root.querySelector('.chrome')) + h(errorLine()) + h(this.panel.element) + h(this.front.querySelector('.addbar')) + grid.offsetHeight);
    if (Math.abs(height - window.innerHeight) < 2 || height === this.lastRequestedHeight) return;
    this.lastRequestedHeight = height;
    void call('fit_window', { height }).catch((error) => console.warn('Vindictive: fit_window', error));
  }

  /** A resize that did not come from `fitWindow` lets the next render re-fit. */
  private userResized(): void {
    if (Math.abs(window.innerHeight - this.lastRequestedHeight) > 2) this.lastRequestedHeight = 0;
  }

  /** Re-fit titles after the window stops resizing (the tile unit follows the width). */
  private scheduleFit(): void {
    if (this.fitTimer) clearTimeout(this.fitTimer);
    this.fitTimer = setTimeout(() => fitTitles(this.front), 80);
  }

  private renderBack(state: UiState, now: Date): void {
    const board = state.board;
    if (!board) return;
    if (state.view.kind === 'settings') {
      mount(this.back, renderSettings({ settings: board.settings, actions: this.settingsActions() }));
      return;
    }
    if (state.view.kind === 'edit') {
      this.renderEdit(board, state.view.id);
      return;
    }
    if (state.view.kind === 'detail') {
      const id = state.view.id;
      const tip = board.tips.find((t) => t.id === id);
      if (tip) mount(this.back, renderDetail({ tip, settings: board.settings, now, actions: this.detailActions(), isNextUp: tip.id === board.next_up }));
      return;
    }
  }

  /**
   * The edit form is mounted once and then left alone: folder rescans and the
   * countdown tick must not wipe what the user is typing. A change to the
   * file on disk surfaces as a conflict when saving.
   */
  private renderEdit(board: BoardState, id: string): void {
    const mounted = this.back.querySelector<HTMLElement>('.edit-form');
    if (mounted?.dataset.editing === id) return;
    const tip = board.tips.find((t) => t.id === id);
    if (tip) mount(this.back, renderEditForm({ tip, actions: this.editActions(tip) }));
  }

  private editActions(tip: Tip) {
    const id = tip.id;
    // The stamp the form starts from: a newer file on disk is a conflict.
    // After a failed save the form adopts the current stamp, so saving
    // again (as the conflict message suggests) is a deliberate overwrite.
    let base = tip.sha ?? null;
    return {
      onSave: async (edit: TipEdit) => {
        try {
          this.applyBoard(await call('update_tip', { id, base, edit }));
        } catch (error) {
          base = this.store.get().board?.tips.find((t) => t.id === id)?.sha ?? base;
          throw error;
        }
        this.store.set({ view: { kind: 'detail', id } });
      },
      onCancel: () => this.store.set({ view: { kind: 'detail', id } }),
    };
  }

  private back_(): void {
    closeLightbox();
    this.store.set({ view: { kind: 'board' } });
  }

  private addBarActions() {
    return {
      onSubmit: async (text: string) => {
        const board = await call('create_tip', { text });
        this.applyBoard(board);
        this.store.set({ adding: false });
      },
      onCancel: () => this.store.set({ adding: false }),
    };
  }

  private chromeActions() {
    return {
      onSync: () => void this.refresh('sync_now'),
      onOpenFolder: () => void guard('open_folder', () => call('open_folder')),
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
      onDelete: (id: string) => void this.mutate(() => call('delete_tip', { id }), 'delete_tip').then(() => this.back_()),
      onSetColor: (id: string, color: string | null) => void this.mutate(() => call('set_color', { id, color }), 'set_color'),
      onSetSize: (id: string, size: TileSize | null) => void this.mutate(() => call('set_size', { id, size }), 'set_size'),
      onEdit: (id: string) => this.store.set({ view: { kind: 'edit', id } }),
      onEditLocal: (id: string) =>
        void guard('edit_local', () => call('edit_local', { id })).then((path) => {
          if (path) showNotice(t('openedIn'), path);
        }),
      onReveal: (id: string) => void guard('reveal_tip', () => call('reveal_tip', { id })),
      onOpenLink: (url: string) => void guard('open link', () => openExternal(url)),
      onBack: () => this.back_(),
      resolveImage: this.resolveImage,
    };
  }

  private settingsActions() {
    return {
      onSave: async (settings: Settings) => {
        const board = await call('save_settings', { settings });
        this.applyBoard(board);
      },
      onPickFolder: () => call('pick_folder'),
      onOpenFolder: () => call('open_folder'),
      onClose: () => this.back_(),
    };
  }

  openSettings(): void {
    this.store.set({ view: { kind: 'settings' } });
  }
}
