import type { BoardState, Tip } from '../types';
import { t } from '../lib/i18n';
import type { ImageResolver } from '../lib/markdown';
import { applyManualOrder, orderForMove } from '../lib/order';
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
  onMenu?: (id: string, x: number, y: number) => void;
  /** Click on the trailing "+" tile. When absent, no add tile is rendered. */
  onAdd?: () => void;
  /** Open Settings from the first-run message. */
  onSetup?: () => void;
  /** A tile was dropped somewhere else: persist its new `order`. */
  onReorder?: (id: string, order: number) => void;
  /**
   * Ids already shown on the board. Tiles not in the set play the entry
   * animation and are added to it; pass nothing to animate every tile.
   */
  seen?: Set<string>;
  resolveImage?: ImageResolver;
}

export function clampColumns(value: number | undefined): number {
  if (!value || Number.isNaN(value)) return DEFAULT_COLUMNS;
  return Math.min(MAX_COLUMNS, Math.max(MIN_COLUMNS, Math.round(value)));
}

function isSnoozed(tip: Tip, now: Date): boolean {
  const until = parseNaive(tip.snoozed_until_at);
  return !!until && until > now;
}

/** Tips the board should show, honouring the "show done" setting and manual order. */
export function visibleTips(state: BoardState, now: Date): Tip[] {
  const shown = state.tips.filter((tip) => {
    if (tip.status === 'done') return state.settings.show_done;
    return !isSnoozed(tip, now);
  });
  return applyManualOrder(shown);
}

/**
 * HTML5 drag and drop between tiles. The drop position is "before" the
 * hovered tile when the pointer is in its left half, "after" otherwise;
 * dropping on empty grid space moves the tile to the end.
 */
function wireDragAndDrop(grid: HTMLElement, ranked: readonly Tip[], onReorder: (id: string, order: number) => void): void {
  let dragged: string | undefined;
  const tiles = (): HTMLElement[] => Array.from(grid.querySelectorAll<HTMLElement>('.tile:not(.tile-add)'));
  const clearMarks = (): void => tiles().forEach((t) => t.classList.remove('tile-drop-before', 'tile-drop-after'));
  const target = (event: DragEvent): { tile: HTMLElement; after: boolean } | undefined => {
    const tile = (event.target as HTMLElement | null)?.closest<HTMLElement>('.tile:not(.tile-add)');
    if (!tile || tile.dataset.id === dragged) return undefined;
    const rect = tile.getBoundingClientRect();
    return { tile, after: event.clientX > rect.left + rect.width / 2 };
  };
  grid.addEventListener('dragstart', (event) => {
    const tile = (event.target as HTMLElement | null)?.closest<HTMLElement>('.tile:not(.tile-add)');
    if (!tile?.dataset.id) return;
    dragged = tile.dataset.id;
    tile.classList.add('tile-dragging');
    event.dataTransfer?.setData('text/plain', dragged);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  });
  grid.addEventListener('dragover', (event) => {
    if (!dragged) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    clearMarks();
    const t = target(event);
    if (t) t.tile.classList.add(t.after ? 'tile-drop-after' : 'tile-drop-before');
  });
  grid.addEventListener('dragleave', (event) => {
    if (event.target === grid) clearMarks();
  });
  grid.addEventListener('drop', (event) => {
    if (!dragged) return;
    event.preventDefault();
    const id = dragged;
    const t = target(event);
    const others = tiles().filter((x) => x.dataset.id !== id);
    let index = others.length;
    if (t) {
      const i = others.indexOf(t.tile);
      index = i < 0 ? others.length : i + (t.after ? 1 : 0);
    }
    clearMarks();
    grid.querySelector('.tile-dragging')?.classList.remove('tile-dragging');
    dragged = undefined;
    const order = orderForMove(ranked, id, index);
    if (order !== undefined) onReorder(id, order);
  });
  grid.addEventListener('dragend', () => {
    clearMarks();
    grid.querySelector('.tile-dragging')?.classList.remove('tile-dragging');
    dragged = undefined;
  });
}

/** The trailing "+" tile. */
function addTile(onAdd: () => void): HTMLButtonElement {
  const button = el(
    'button',
    { className: 'tile tile-sm tile-add', type: 'button', 'aria-label': t('addTip'), title: t('addTip') },
    el('span', { className: 'tile-add-glyph', 'aria-hidden': 'true', text: '+' }),
  );
  button.addEventListener('click', onAdd);
  return button;
}

/** A folder is set and readable. */
function isReady(state: BoardState): boolean {
  return state.settings.folder.trim().length > 0 && !state.sync_error;
}

function emptyMessage(state: BoardState, onSetup?: () => void): HTMLElement {
  if (isReady(state) || !onSetup) return el('p', { className: 'board-empty', text: t('emptyReady') });
  const button = el('button', { type: 'button', className: 'action action-primary', text: t('chooseFolder') });
  button.addEventListener('click', onSetup);
  return el('div', { className: 'board-empty' }, el('p', { text: t('emptyNoFolder') }), button);
}

/** Render the tile grid into a fresh element. */
export function renderBoard(options: BoardOptions): HTMLElement {
  const { state, now, onOpen, onMenu, onAdd, onSetup, onReorder, seen, resolveImage } = options;
  const columns = clampColumns(state.settings.columns);
  const grid = el('div', { className: 'board', role: 'list' });
  grid.style.setProperty('--cols', String(columns));

  const tips = visibleTips(state, now);
  if (tips.length === 0) {
    mount(grid, emptyMessage(state, onSetup), onAdd && isReady(state) ? addTile(onAdd) : null);
    return grid;
  }
  let entering = 0;
  const tiles = tips.map((tip) => {
    const fresh = !seen || !seen.has(tip.id);
    const enterIndex = fresh ? entering++ : undefined;
    seen?.add(tip.id);
    return renderTile({ tip, isNextUp: tip.id === state.next_up, now, columns, onOpen, onMenu, enterIndex, resolveImage });
  });
  mount(grid, ...tiles, onAdd ? addTile(onAdd) : null);
  if (onReorder) {
    // Keys come from the backend ranking (before manual order is applied).
    const ranked = state.tips.filter((tip) => tips.some((t) => t.id === tip.id));
    wireDragAndDrop(grid, ranked, onReorder);
  }
  return grid;
}

/** Title size tiers, largest first; see `tile.css`. */
export const TITLE_TIERS = ['tile-title-xl', 'tile-title-lg'] as const;

/**
 * Give every title the largest tier that does not overflow its tile. Must run
 * after the grid is in the document (layout is needed to detect overflow).
 * A clamped title reports `scrollHeight` larger than `clientHeight`.
 */
export function fitTitles(grid: ParentNode): void {
  for (const title of Array.from(grid.querySelectorAll<HTMLElement>('.tile-title'))) {
    title.classList.remove(...TITLE_TIERS);
    for (const tier of TITLE_TIERS) {
      title.classList.add(tier);
      if (title.scrollHeight <= title.clientHeight + 1) break;
      title.classList.remove(tier);
    }
  }
}

/** Arrow-key navigation between tiles. */
export function handleBoardKeys(grid: HTMLElement, event: KeyboardEvent): void {
  const keys = ['ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp'];
  if (!keys.includes(event.key)) return;
  const tiles = Array.from(grid.querySelectorAll<HTMLButtonElement>('.tile:not(.tile-add)'));
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
