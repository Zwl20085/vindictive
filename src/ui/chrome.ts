import type { BoardState, Dock } from '../types';
import { t } from '../lib/i18n';
import { formatClock, parseNaive } from '../lib/time';
import { el } from './dom';
import { showMenu } from './menu';
import type { SyncStatus } from './store';

export interface ChromeActions {
  onSync: () => void;
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

function buildMenu(options: ChromeOptions, x: number, y: number): void {
  const { state, actions } = options;
  const dock = state?.settings.dock;
  showMenu(x, y, [
    { label: t('menuSync'), onSelect: actions.onSync },
    { label: t('menuSettings'), onSelect: actions.onSettings },
    { label: t('menuShowDone'), checked: state?.settings.show_done, onSelect: actions.onToggleDone },
    { label: t('menuDockLeft'), checked: dock === 'left', onSelect: () => actions.onDock('left') },
    { label: t('menuDockRight'), checked: dock === 'right', onSelect: () => actions.onDock('right') },
    { label: t('menuFree'), checked: dock === 'free', onSelect: () => actions.onDock('free') },
    { label: t('menuQuit'), onSelect: actions.onQuit },
  ]);
}

/** The 20 px drag strip with the sync dot. */
export function renderChrome(options: ChromeOptions): HTMLElement {
  const { state, sync } = options;
  const status: SyncStatus = state?.sync_error ? 'error' : sync;
  const dot = el('span', {
    className: 'sync-dot',
    'data-state': status,
    role: 'status',
    'aria-label': syncTitle(state, sync),
  });
  const strip = el(
    'header',
    { className: 'chrome', 'data-tauri-drag-region': true, title: syncTitle(state, sync) },
    dot,
    el('span', { className: 'chrome-title', 'data-tauri-drag-region': true, text: BRAND }),
    el('span', { className: 'chrome-hatch', 'data-tauri-drag-region': true, 'aria-hidden': 'true' }),
  );
  strip.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    buildMenu(options, event.clientX, event.clientY);
  });
  dot.addEventListener('click', options.actions.onSync);
  return strip;
}
