import type { BoardState, Dock } from '../types';
import { t } from '../lib/i18n';
import { formatClock, parseNaive } from '../lib/time';
import { el } from './dom';
import { showMenu } from './menu';
import type { SyncStatus } from './store';

export interface ChromeActions {
  onSync: () => void;
  onOpenFolder: () => void;
  onSettings: () => void;
  onToggleDone: () => void;
  onDock: (dock: Dock) => void;
  onQuit: () => void;
}

export interface ChromeOptions {
  state?: BoardState;
  sync: SyncStatus;
  actions: ChromeActions;
}

/** Brand mark on the strip; deliberately not translated. */
export const BRAND = 'VINDICTIVE';

function syncTitle(state: BoardState | undefined, sync: SyncStatus): string {
  if (sync === 'syncing') return t('syncing');
  if (state?.sync_error) return `${t('syncError')}: ${state.sync_error}`;
  const last = parseNaive(state?.last_sync);
  return last ? `${t('synced')} ${formatClock(last)}` : t('notSynced');
}

function buildMenu(options: ChromeOptions, x: number, y: number, returnFocus?: HTMLElement): void {
  const { state, actions } = options;
  const dock = state?.settings.dock;
  showMenu(
    x,
    y,
    [
      { label: t('menuSync'), onSelect: actions.onSync },
      { label: t('openFolder'), onSelect: actions.onOpenFolder },
      { label: t('menuSettings'), onSelect: actions.onSettings },
      { label: t('menuShowDone'), checked: state?.settings.show_done, onSelect: actions.onToggleDone },
      { label: t('menuDockLeft'), checked: dock === 'left', onSelect: () => actions.onDock('left') },
      { label: t('menuDockRight'), checked: dock === 'right', onSelect: () => actions.onDock('right') },
      { label: t('menuFree'), checked: dock === 'free', onSelect: () => actions.onDock('free') },
      { label: t('menuQuit'), onSelect: actions.onQuit },
    ],
    returnFocus,
  );
}

/**
 * The 20 px drag strip with the sync square. The square is a button: click,
 * Enter or Space reloads the tips folder; the menu key or Shift+F10 on it opens
 * the strip menu under it, and right-click anywhere on the strip opens the same
 * menu at the pointer. The sync state is announced through a visually hidden
 * live region.
 */
export function renderChrome(options: ChromeOptions): HTMLElement {
  const { state, sync } = options;
  const status: SyncStatus = state?.sync_error ? 'error' : sync;
  const title = syncTitle(state, sync);
  const dot = el('button', {
    type: 'button',
    className: 'sync-dot',
    'data-state': status,
    'aria-label': `${title} · ${t('menuSync')}`,
    'aria-haspopup': 'menu',
    title,
  });
  const live = el('span', { className: 'visually-hidden', role: 'status', 'aria-live': 'polite', text: title });
  const strip = el(
    'header',
    { className: 'chrome', 'data-tauri-drag-region': true, title },
    dot,
    live,
    el('span', { className: 'chrome-title', 'data-tauri-drag-region': true, text: BRAND }),
  );
  strip.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    // The keyboard menu key reports no pointer position: open under the square.
    if (event.clientX === 0 && event.clientY === 0) {
      const r = dot.getBoundingClientRect();
      buildMenu(options, r.left, r.bottom, dot);
    } else buildMenu(options, event.clientX, event.clientY);
  });
  dot.addEventListener('click', options.actions.onSync);
  dot.addEventListener('keydown', (event) => {
    if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
      // Handled here, so the browser does not also fire a contextmenu event.
      event.preventDefault();
      const r = dot.getBoundingClientRect();
      buildMenu(options, r.left, r.bottom, dot);
    }
  });
  return strip;
}
