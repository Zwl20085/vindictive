import { call, isTauri, on, onBoardUpdated, openExternal } from '../api';
import type { TileSize } from '../lib/tileSize';
import type { BoardState, Dock, Settings, Tip, TipEdit } from '../types';
import { parseCapture } from '../lib/capture-hint';
import { setLocale, t } from '../lib/i18n';
import { devClawdMood } from '../lib/mood';
import { renderAddBar } from './addbar';
import { closeMenuIfOrphaned, isMenuOpen, showMenu } from './menu';
import { createBoard, fitTitles, handleBoardKeys, updateBoard } from './board';
import { renderChrome, updateChrome } from './chrome';
import { refreshDetail, renderDetail } from './detail';
import { el, mount } from './dom';
import { emptyDraft, isEditFormDirty, renderEditForm, showEditFormNotice, tipFromDraft } from './edit-form';
import { errorLine, guard, showError, showNotice } from './errors';
import { cachedResolver, pruneImageCache } from './images';
import { closeLightbox, isLightboxOpen } from './lightbox';
import { Panel } from './panel';
import { DEFAULT_WINDOW_OPACITY, renderSettings, settingsEscape } from './settings';
import { Store, type EditOrigin, type UiState } from './store';
import { tileMenuItems } from './tile-menu';

/** First-load retries while the backend finishes starting. */
export const STARTUP_ATTEMPTS = 20;
export const STARTUP_RETRY_MS = 150;

/** Re-render countdowns this often. */
export const TICK_MS = 30_000;
/** Ask the backend for fresh weather this often (it caches on its own too). */
export const WEATHER_MS = 20 * 60_000;
/** How long an `app-sized` announcement explains a following window resize. */
export const SIZED_GRACE_MS = 1500;

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
  /**
   * What the back face currently holds (`settings`, `edit:<id>`,
   * `detail:<id>:<sha>:<next>:<lang>`), or '' once the view returned to the
   * board. The back face keeps its content while flipping away, so the DOM
   * alone cannot tell a fresh open from a re-render.
   */
  private backKey = '';
  /** Tip object behind the shown detail, for tips without a `sha`. */
  private backTip: Tip | undefined;
  /** Draft behind the mounted create form; a new draft (a new open) remounts it. */
  private backDraft: TipEdit | undefined;

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
    // The backend announces every resize it makes (startup, dock, fit, clamp).
    await on('app-sized', (size) => this.sizeAnnounced(size.height)).catch((e) => console.warn('Vindictive: app-sized', e));
    await this.loadInitialState();
    this.openFromHash();
    setInterval(() => this.renderFaces(this.store.get()), TICK_MS);
    setInterval(() => void this.loadWeather(true), WEATHER_MS);
    this.lastSeenHeight = window.innerHeight;
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
    document.documentElement.style.setProperty('--alpha', String(Math.min(100, Math.max(20, board.settings.window_opacity || DEFAULT_WINDOW_OPACITY)) / 100));
    setLocale(board.settings.language);
    let view = this.store.get().view;
    const id = 'id' in view ? view.id : undefined;
    if (id !== undefined && !board.tips.some((t) => t.id === id)) {
      // The tip left the folder (deleted, renamed, another PC). A figure of
      // it must not stay over the board; an edit with unsaved text stays
      // open with a notice (its save then fails as a conflict), anything
      // else returns to the board.
      closeLightbox();
      const form = view.kind === 'edit' ? this.back.querySelector<HTMLFormElement>('form.edit-form') : null;
      if (form?.dataset.editing === id && isEditFormDirty(form)) showEditFormNotice(form, t('tipGone'));
      else view = { kind: 'board' };
    }
    this.store.set({ board, sync: board.sync_error ? 'error' : 'ok', view });
    pruneImageCache(new Set(board.tips.map((tip) => tip.sha).filter((sha): sha is string => !!sha)));
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

  /** Run a backend mutation; errors are shown by `guard`. True on success. */
  private async mutate(action: () => Promise<BoardState>, context: string): Promise<boolean> {
    const board = await guard(context, action);
    if (!board) return false;
    this.applyBoard(board);
    return true;
  }

  private onKey(event: KeyboardEvent): void {
    if (isLightboxOpen()) return; // the lightbox handles Esc itself
    if (isMenuOpen()) return; // keys belong to the menu, not the board behind it
    const { view, adding, board } = this.store.get();
    if (view.kind === 'edit' || view.kind === 'create') return; // the form handles Esc and Ctrl+Enter
    const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
    if (event.key === 'Escape' && view.kind !== 'board') {
      // Settings with unsaved changes: the first Esc only warns, like the edit form.
      const form = view.kind === 'settings' ? this.back.querySelector<HTMLFormElement>('form.settings:not(.edit-form)') : null;
      if (form && board && !settingsEscape(form, board.settings)) {
        event.preventDefault();
        return;
      }
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
    // Owned by the tile: a board update that replaces it closes the menu.
    const tile = Array.from(this.front.querySelectorAll<HTMLElement>('.tile[data-id]')).find((node) => node.dataset.id === id);
    if (tip) showMenu(x, y, tileMenuItems(tip, this.detailActions('board'), x, y), tile);
  }

  /**
   * The strip is updated in place (focus on the sync square and a click in
   * progress survive store updates); only a language switch rebuilds it.
   */
  private renderChrome(state: UiState): void {
    const slot = this.root.querySelector('.chrome-slot');
    if (!slot) return;
    const language = state.board?.settings.language ?? 'en';
    const options = { state: state.board, sync: state.sync, actions: this.chromeActions() };
    const strip = slot.querySelector<HTMLElement>(':scope > .chrome');
    if (strip && strip.dataset.lang === language) {
      updateChrome(strip, options);
      return;
    }
    const fresh = renderChrome(options);
    fresh.dataset.lang = language;
    mount(slot, fresh);
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
    const onBoard = state.view.kind === 'board';
    // The face turned away must not be reachable by Tab or screen readers.
    this.front.toggleAttribute('inert', !onBoard);
    this.back.toggleAttribute('inert', onBoard);
    this.renderFront(state, now);
    this.renderBack(state, now);
    this.stage.classList.toggle('open', !onBoard);
    closeMenuIfOrphaned();
    this.noteLayout(state);
    this.fitWindow();
  }

  /**
   * Front face: add bar (if open) above the board grid. Both nodes are kept
   * across renders: the add bar so the tick does not wipe what is being
   * typed, the grid so `updateBoard` can reuse unchanged tiles.
   */
  private renderFront(state: UiState, now: Date): void {
    const board = state.board;
    if (!board) return;
    const active = document.activeElement;
    const focusedId = active instanceof HTMLElement && this.front.contains(active) ? active.closest<HTMLElement>('.tile[data-id]')?.dataset.id : undefined;

    let bar = this.front.querySelector<HTMLElement>(':scope > .addbar');
    if (!state.adding) bar = null;
    else if (!bar) bar = renderAddBar(this.addBarActions());
    const grid = this.front.querySelector<HTMLElement>(':scope > .board') ?? createBoard();
    const built = updateBoard(grid, {
      state: board,
      now,
      seen: this.seen,
      resolveImage: this.resolveImage,
      onOpen: (id) => this.store.set({ view: { kind: 'detail', id } }),
      onMenu: (id, x, y) => this.tileMenu(id, x, y),
      onAdd: () => this.store.set({ adding: true }),
      onSetup: () => this.store.set({ view: { kind: 'settings' } }),
      onReorder: (id, order) => void this.mutate(() => call('set_order', { id, order }), 'set_order'),
    });
    const wanted = bar ? [bar, grid] : [grid];
    for (const child of Array.from(this.front.children)) {
      if (!wanted.includes(child as HTMLElement)) child.remove();
    }
    wanted.forEach((node, index) => {
      if (this.front.children[index] !== node) this.front.insertBefore(node, this.front.children[index] ?? null);
    });
    built.forEach((tile) => fitTitles(tile));
    // A replaced or moved tile loses focus; put it back on the same tip.
    if (focusedId && !this.front.contains(document.activeElement)) this.focusTile(focusedId);
  }

  /** Focus the board tile for `id`, if it is shown. */
  private focusTile(id: string): void {
    const tile = Array.from(this.front.querySelectorAll<HTMLElement>('.tile[data-id]')).find((t) => t.dataset.id === id);
    tile?.focus();
  }

  private fitTimer: ReturnType<typeof setTimeout> | undefined;
  private lastRequestedHeight = 0;
  /** Window height at the last resize event, to tell real height changes from width-only ones. */
  private lastSeenHeight = 0;
  /** What the fitted height depends on: view kind, add bar, number of tiles shown. */
  private layoutKey = '';
  /**
   * The layout key at which the user resized the window by hand. While the
   * layout stays the same, fit height leaves their height alone.
   */
  private userHeightFor: string | undefined;

  /** Track the layout; a change ends a manual height override. */
  private noteLayout(state: UiState): void {
    const tiles = this.front.querySelectorAll(':scope > .board > .tile[data-id]').length;
    this.layoutKey = `${state.view.kind}|${state.adding}|${tiles}`;
    if (this.userHeightFor !== undefined && this.userHeightFor !== this.layoutKey) {
      this.userHeightFor = undefined;
      this.lastRequestedHeight = 0;
    }
  }

  /** Ask the backend to size the window to the board (setting `fit_height`). */
  private fitWindow(): void {
    const state = this.store.get();
    if (!state.board?.settings.fit_height || state.view.kind !== 'board') return;
    if (this.userHeightFor !== undefined) return; // the user chose this height
    const grid = this.front.querySelector<HTMLElement>('.board');
    if (!grid) return;
    const h = (el: Element | null | undefined): number => (el instanceof HTMLElement && !el.hidden ? el.offsetHeight : 0);
    const bar = this.front.querySelector<HTMLElement>(':scope > .addbar');
    // offsetHeight leaves out margins; the add bar has a top margin (board.css).
    const barMargin = bar ? Number.parseFloat(getComputedStyle(bar).marginTop) || 0 : 0;
    const height = Math.ceil(h(this.root.querySelector('.chrome')) + h(errorLine()) + h(this.panel.element) + h(bar) + barMargin + grid.offsetHeight);
    if (Math.abs(height - window.innerHeight) < 2 || height === this.lastRequestedHeight) return;
    this.lastRequestedHeight = height;
    void call('fit_window', { height })
      .then(() => (this.sizingSeen = true))
      .catch((error) => console.warn('Vindictive: fit_window', error));
  }

  /** Heights the backend announced (`app-sized`) just before resizing the window itself. */
  private announcedHeights: { height: number; at: number }[] = [];
  /**
   * An `app-sized` or a `fit_window` round-trip has happened. Before that the
   * window is still being set up (startup sizing), so no resize is the user's.
   */
  private sizingSeen = false;

  private sizeAnnounced(height: number): void {
    this.sizingSeen = true;
    const now = Date.now();
    this.announcedHeights = [...this.announcedHeights.filter((a) => now - a.at <= SIZED_GRACE_MS), { height, at: now }];
  }

  /**
   * A height change is the user's unless it was announced by the backend
   * (`app-sized`, within `SIZED_GRACE_MS`), answers our own `fit_window`
   * request, or comes before any sizing round-trip (startup). A user height is
   * kept until the layout changes (see `noteLayout`); a programmatic one that
   * is not our fit (dock, startup sizing) is fitted again.
   */
  private userResized(): void {
    const height = window.innerHeight;
    const changed = Math.abs(height - this.lastSeenHeight) > 2;
    this.lastSeenHeight = height;
    if (!changed) return;
    if (this.lastRequestedHeight && Math.abs(height - this.lastRequestedHeight) <= 2) return;
    const now = Date.now();
    const announced = this.announcedHeights.some((a) => now - a.at <= SIZED_GRACE_MS && Math.abs(height - a.height) <= 2);
    if (announced || !this.sizingSeen) {
      this.lastRequestedHeight = 0;
      this.fitWindow();
      return;
    }
    this.userHeightFor = this.layoutKey;
  }

  /** Re-fit titles after the window stops resizing (the tile unit follows the width). */
  private scheduleFit(): void {
    if (this.fitTimer) clearTimeout(this.fitTimer);
    this.fitTimer = setTimeout(() => fitTitles(this.front), 80);
  }

  private renderBack(state: UiState, now: Date): void {
    const board = state.board;
    if (!board) return;
    const view = state.view;
    if (view.kind === 'board') {
      // Leave the content in place for the flip back; the next open re-renders.
      this.backKey = '';
      this.backTip = undefined;
      this.backDraft = undefined;
      return;
    }
    if (view.kind === 'settings') {
      // Mounted once per opening (fresh settings then), so the tick and
      // board updates never wipe a half-typed form. A language switch
      // re-renders it so the labels follow.
      const key = `settings:${board.settings.language}`;
      if (this.backKey === key && this.back.querySelector('.settings')) return;
      this.backKey = key;
      mount(this.back, renderSettings({ settings: board.settings, actions: this.settingsActions() }));
      return;
    }
    if (view.kind === 'edit') {
      this.renderEdit(board, view.id, view.from ?? 'detail');
      return;
    }
    if (view.kind === 'create') {
      this.renderCreate(view.draft);
      return;
    }
    const tip = board.tips.find((t) => t.id === view.id);
    if (!tip) return;
    this.renderDetailFace(tip, board, now);
  }

  /**
   * The detail panel is rebuilt only when its tip changed (new `sha`, or a
   * new object for tips without one), next-up moved, or the language or
   * the highlight words changed. Otherwise only its countdown is refreshed, so scroll position,
   * an armed delete and running animations survive board updates.
   */
  private renderDetailFace(tip: Tip, board: BoardState, now: Date): void {
    const isNextUp = tip.id === board.next_up;
    // Highlight words change what the body marks, so a new list rebuilds the panel too.
    const highlights = (board.settings.highlight_words ?? []).join('\n');
    const key = `detail:${tip.id}:${tip.sha ?? ''}:${isNextUp}:${board.settings.language}:${highlights}`;
    const mounted = this.back.querySelector<HTMLElement>(':scope > .detail');
    const sameTip = tip.sha ? mounted?.dataset.sha === tip.sha : this.backTip === tip;
    if (mounted && this.backKey === key && sameTip) {
      refreshDetail(mounted, tip, now);
      return;
    }
    const opening = !this.backKey.startsWith(`detail:${tip.id}:`);
    this.backKey = key;
    this.backTip = tip;
    const panel = renderDetail({ tip, settings: board.settings, now, actions: this.detailActions(), isNextUp });
    mount(this.back, panel);
    if (opening) panel.focus({ preventScroll: true });
  }

  /**
   * The edit form is mounted once and then left alone: folder rescans and the
   * countdown tick must not wipe what the user is typing. A change to the
   * file on disk surfaces as a conflict when saving.
   */
  private renderEdit(board: BoardState, id: string, from: EditOrigin): void {
    const mounted = this.back.querySelector<HTMLElement>('.edit-form');
    const key = `edit:${id}:${from}`;
    if (mounted?.dataset.editing === id && this.backKey === key) return;
    const tip = board.tips.find((t) => t.id === id);
    if (!tip) return;
    this.backKey = key;
    this.backTip = undefined;
    mount(this.back, renderEditForm({ tip, actions: this.editActions(tip, from) }));
  }

  /** The create form: mounted once per opening, like the edit form, so nothing typed is lost. */
  private renderCreate(draft: TipEdit): void {
    const mounted = this.back.querySelector<HTMLElement>('.edit-form');
    if (mounted?.dataset.editing === 'new' && this.backKey === 'create' && this.backDraft === draft) return;
    this.backKey = 'create';
    this.backTip = undefined;
    this.backDraft = draft;
    const form = renderEditForm({ tip: tipFromDraft(draft), mode: 'create', actions: this.createActions() });
    mount(this.back, form);
    form.querySelector<HTMLInputElement>('[name="title"]')?.focus({ preventScroll: true });
  }

  /** The full form for a new tip, prefilled from the add bar's text. */
  private expand(text: string): void {
    this.store.set({ view: { kind: 'create', draft: { ...emptyDraft(), ...parseCapture(text) } } });
  }

  private createActions() {
    return {
      // Not through `mutate`: like the edit form, a failure stays in the form
      // (shown inline, text kept) instead of only on the error line.
      onSave: async (edit: TipEdit) => {
        const before = new Set(this.store.get().board?.tips.map((t) => t.id));
        this.applyBoard(await call('create_tip_full', { edit }));
        const tips = this.store.get().board?.tips ?? [];
        const created = tips.find((t) => !before.has(t.id) && t.title === edit.title) ?? tips.find((t) => t.title === edit.title);
        this.store.set({ view: { kind: 'board' }, adding: false });
        if (created) this.focusTile(created.id);
      },
      // Back to the board with the add bar and its text as they were.
      onCancel: () => {
        this.store.set({ view: { kind: 'board' } });
        this.front.querySelector<HTMLInputElement>('.addbar-input')?.focus();
      },
    };
  }

  /** Save and Cancel return to where the edit was opened: the detail, or the board (tile menu). */
  private editActions(tip: Tip, from: EditOrigin) {
    const id = tip.id;
    const leave = (): void => {
      const exists = this.store.get().board?.tips.some((t) => t.id === id);
      if (from === 'detail' && exists) this.store.set({ view: { kind: 'detail', id } });
      else this.back_();
    };
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
        leave();
      },
      onCancel: leave,
    };
  }

  /**
   * After Done / Snooze / Delete: flip back on success. On failure (the error
   * is already shown) rebuild the detail, whose actions were disabled for the
   * leaving animation.
   */
  private afterLeave(ok: boolean): void {
    if (ok) {
      this.back_();
      return;
    }
    this.backKey = '';
    this.renderFaces(this.store.get());
  }

  private back_(): void {
    closeLightbox();
    const view = this.store.get().view;
    const id = 'id' in view ? view.id : undefined;
    this.store.set({ view: { kind: 'board' } });
    // Return keyboard focus to the tile the detail was opened from.
    if (id) this.focusTile(id);
  }

  private addBarActions() {
    return {
      onSubmit: async (text: string) => {
        const board = await call('create_tip', { text });
        this.applyBoard(board);
        this.store.set({ adding: false });
      },
      onExpand: (text: string) => this.expand(text),
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

  /** `from`: where an Edit opened from these actions returns to. */
  private detailActions(from: EditOrigin = 'detail') {
    return {
      onDone: (id: string) => void this.mutate(() => call('mark_done', { id }), 'mark_done').then((ok) => this.afterLeave(ok)),
      onReopen: (id: string) => void this.mutate(() => call('reopen', { id }), 'reopen'),
      onSnooze: (id: string, minutes: number) => void this.mutate(() => call('snooze', { id, minutes }), 'snooze').then((ok) => this.afterLeave(ok)),
      onDelete: (id: string) => void this.mutate(() => call('delete_tip', { id }), 'delete_tip').then((ok) => this.afterLeave(ok)),
      onSetColor: (id: string, color: string | null) => void this.mutate(() => call('set_color', { id, color }), 'set_color'),
      onSetSize: (id: string, size: TileSize | null) => void this.mutate(() => call('set_size', { id, size }), 'set_size'),
      onEdit: (id: string) => this.store.set({ view: { kind: 'edit', id, from } }),
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
