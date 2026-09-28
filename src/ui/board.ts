import type { BoardState, Tip } from '../types';
import { parseNaive } from '../lib/time';
import { el, mount } from './dom';
import { renderTile } from './tile';

export const MIN_COLUMNS = 2;
export const MAX_COLUMNS = 6;
export const DEFAULT_COLUMNS = 4;

export interface BoardOptions {
  state: BoardState;
  now: Date;
  onOpen: (id: string) => void;
}

export function clampColumns(value: number | undefined): number {
  if (!value || Number.isNaN(value)) return DEFAULT_COLUMNS;
  return Math.min(MAX_COLUMNS, Math.max(MIN_COLUMNS, Math.round(value)));
}

function isSnoozed(tip: Tip, now: Date): boolean {
  const until = parseNaive(tip.snoozed_until_at);
  return !!until && until > now;
}

/** Tips the board should show, honouring the "show done" setting. */
export function visibleTips(state: BoardState, now: Date): Tip[] {
  return state.tips.filter((tip) => {
    if (tip.status === 'done') return state.settings.show_done;
    return !isSnoozed(tip, now);
  });
}

function emptyMessage(state: BoardState): HTMLElement {
  const text = state.has_token
    ? 'Nothing to do. Add a tip with the hotkey or on GitHub.'
    : 'Open Settings (right-click the top strip) and add your GitHub token.';
  return el('p', { className: 'board-empty', text });
}

/** Render the tile grid into a fresh element. */
export function renderBoard(options: BoardOptions): HTMLElement {
  const { state, now, onOpen } = options;
  const columns = clampColumns(state.settings.columns);
  const grid = el('div', { className: 'board', role: 'list' });
  grid.style.setProperty('--cols', String(columns));

  const tips = visibleTips(state, now);
  if (tips.length === 0) {
    mount(grid, emptyMessage(state));
    return grid;
  }
  const tiles = tips.map((tip) =>
    renderTile({ tip, isNextUp: tip.id === state.next_up, now, columns, onOpen }),
  );
  mount(grid, ...tiles);
  return grid;
}

/** Arrow-key navigation between tiles. */
export function handleBoardKeys(grid: HTMLElement, event: KeyboardEvent): void {
  const keys = ['ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp'];
  if (!keys.includes(event.key)) return;
  const tiles = Array.from(grid.querySelectorAll<HTMLButtonElement>('.tile'));
  const index = tiles.findIndex((t) => t === document.activeElement);
  if (index < 0) {
    tiles[0]?.focus();
    return;
  }
  const forward = event.key === 'ArrowRight' || event.key === 'ArrowDown';
  const next = tiles[(index + (forward ? 1 : tiles.length - 1)) % tiles.length];
  next?.focus();
  event.preventDefault();
}
