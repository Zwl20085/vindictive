import type { BoardState, Dock } from '../types';
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

function syncTitle(state: BoardState | undefined, sync: SyncStatus): string {
  if (sync === 'syncing') return 'Syncing…';
  if (state?.sync_error) return `Sync error: ${state.sync_error}`;
  const last = parseNaive(state?.last_sync);
  return last ? `Synced ${formatClock(last)}` : 'Not synced yet';
}

function buildMenu(options: ChromeOptions, x: number, y: number): void {
  const { state, actions } = options;
  const dock = state?.settings.dock;
  showMenu(x, y, [
    { label: 'Sync now', onSelect: actions.onSync },
    { label: 'Settings', onSelect: actions.onSettings },
    { label: 'Show done', checked: state?.settings.show_done, onSelect: actions.onToggleDone },
    { label: 'Dock left', checked: dock === 'left', onSelect: () => actions.onDock('left') },
    { label: 'Dock right', checked: dock === 'right', onSelect: () => actions.onDock('right') },
    { label: 'Free', checked: dock === 'free', onSelect: () => actions.onDock('free') },
    { label: 'Quit', onSelect: actions.onQuit },
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
    el('span', { className: 'chrome-title', 'data-tauri-drag-region': true, text: 'vindictive' }),
  );
  strip.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    buildMenu(options, event.clientX, event.clientY);
  });
  dot.addEventListener('click', options.actions.onSync);
  return strip;
}
