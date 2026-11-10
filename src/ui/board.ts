import type { BoardState, Tip } from '../types';
import { t } from '../lib/i18n';
import { applyManualOrder, orderForMove } from '../lib/order';
import { isQuestion } from '../lib/question';
import { isSnoozed } from '../lib/urgency';
import { el } from './dom';
import type { VersionedResolver } from './images';
import { renderTile, updateTile } from './tile';

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
  resolveImage?: VersionedResolver;
  /**
   * Show only question tips (`questions/`). The "+" tile and reordering are
   * off meanwhile: new tips never go to `questions/`, and an order computed
   * among the shown subset would misplace tiles on the full board.
   */
  questionsOnly?: boolean;
}

/**
 * Per-grid state that outlives one render: drag and drop and the keyboard
 * move read the current ranking from here, so their listeners are attached
 * once per grid element and never duplicated by later updates.
 */
interface GridState {
  ranked: Tip[];
  onReorder?: (id: string, order: number) => void;
  wired: boolean;
  /** Id of the tile being dragged, if a drag is running. */
  dragged?: string;
}

const gridStates = new WeakMap<HTMLElement, GridState>();

function gridState(grid: HTMLElement): GridState {
  let state = gridStates.get(grid);
  if (!state) {
    state = { ranked: [], wired: false };
    gridStates.set(grid, state);
  }
  return state;
}

/**
 * Persist a move and apply it to the local ranking at once, so a second
 * move made before the backend answers starts from the new position rather
 * than the old one.
 */
function commitMove(state: GridState, id: string, order: number): void {
  state.ranked = state.ranked.map((tip) => (tip.id === id ? { ...tip, order } : tip));
  state.onReorder?.(id, order);
}

/** Drop marks and the dragging class on every tile of `grid`. */
function clearDragMarks(grid: HTMLElement): void {
  grid.querySelectorAll('.tile-drop-before, .tile-drop-after').forEach((t) => t.classList.remove('tile-drop-before', 'tile-drop-after'));
}

/** Reset of the grid whose drag is running; `document` listeners call it. */
let endActiveDrag: (() => void) | undefined;
let documentWired = false;

/**
 * A tile replaced mid-drag (a board update) is detached, so its `dragend`
 * never bubbles to the grid. `document` still sees a drop anywhere in the
 * window, and the source node's own listener (attached at dragstart) still
 * hears `dragend`; either one ends the drag.
 */
function wireDocumentDragEnd(): void {
  if (documentWired) return;
  documentWired = true;
  const end = (): void => endActiveDrag?.();
  document.addEventListener('dragend', end);
  document.addEventListener('drop', end);
}

export function clampColumns(value: number | undefined): number {
  if (!value || Number.isNaN(value)) return DEFAULT_COLUMNS;
  return Math.min(MAX_COLUMNS, Math.max(MIN_COLUMNS, Math.round(value)));
}

/** Tips the board should show, honouring the "show done" setting, the
 *  questions filter and manual order. */
export function visibleTips(state: BoardState, now: Date, questionsOnly = false): Tip[] {
  const shown = state.tips.filter((tip) => {
    if (questionsOnly && !isQuestion(tip)) return false;
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
function wireDragAndDrop(grid: HTMLElement): void {
  const state = gridState(grid);
  if (state.wired) return;
  state.wired = true;
  wireDocumentDragEnd();
  const tiles = (): HTMLElement[] => Array.from(grid.querySelectorAll<HTMLElement>('.tile:not(.tile-add)'));
  const reset = (): void => {
    clearDragMarks(grid);
    grid.querySelectorAll('.tile-dragging').forEach((t) => t.classList.remove('tile-dragging'));
    state.dragged = undefined;
    if (endActiveDrag === reset) endActiveDrag = undefined;
  };
  const target = (event: DragEvent): { tile: HTMLElement; after: boolean } | undefined => {
    const tile = (event.target as HTMLElement | null)?.closest<HTMLElement>('.tile:not(.tile-add)');
    if (!tile || tile.dataset.id === state.dragged) return undefined;
    const rect = tile.getBoundingClientRect();
    return { tile, after: event.clientX > rect.left + rect.width / 2 };
  };
  grid.addEventListener('dragstart', (event) => {
    const tile = (event.target as HTMLElement | null)?.closest<HTMLElement>('.tile:not(.tile-add)');
    if (!tile?.dataset.id) return;
    state.dragged = tile.dataset.id;
    endActiveDrag = reset;
    tile.classList.add('tile-dragging');
    // Heard even after a board update has detached this node.
    tile.addEventListener('dragend', reset, { once: true });
    event.dataTransfer?.setData('text/plain', state.dragged);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
  });
  grid.addEventListener('dragover', (event) => {
    if (!state.dragged) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    clearDragMarks(grid);
    const t = target(event);
    if (t) t.tile.classList.add(t.after ? 'tile-drop-after' : 'tile-drop-before');
  });
  grid.addEventListener('dragleave', (event) => {
    if (event.target === grid) clearDragMarks(grid);
  });
  grid.addEventListener('drop', (event) => {
    if (!state.dragged) return;
    event.preventDefault();
    const id = state.dragged;
    const t = target(event);
    const others = tiles().filter((x) => x.dataset.id !== id);
    let index = others.length;
    if (t) {
      const i = others.indexOf(t.tile);
      index = i < 0 ? others.length : i + (t.after ? 1 : 0);
    }
    reset();
    const order = orderForMove(state.ranked, id, index);
    if (order !== undefined) commitMove(state, id, order);
  });
  grid.addEventListener('dragend', reset);
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

function emptyMessage(state: BoardState, onSetup?: () => void, questionsOnly = false): HTMLElement {
  if (questionsOnly && isReady(state)) return el('p', { className: 'board-empty', text: t('questionsEmpty') });
  if (isReady(state) || !onSetup) return el('p', { className: 'board-empty', text: t('emptyReady') });
  const button = el('button', { type: 'button', className: 'action action-primary', text: t('chooseFolder') });
  button.addEventListener('click', onSetup);
  return el('div', { className: 'board-empty' }, el('p', { text: t('emptyNoFolder') }), button);
}

/** An empty `.board` grid element, ready for `updateBoard`. */
export function createBoard(): HTMLElement {
  return el('div', { className: 'board', role: 'group', 'aria-label': t('boardLabel') });
}

/** Render the tile grid into a fresh element. */
export function renderBoard(options: BoardOptions): HTMLElement {
  const grid = createBoard();
  updateBoard(grid, options);
  return grid;
}

/** Make `grid`'s children exactly `nodes`, in order, moving only what is out of place. */
function arrange(grid: HTMLElement, nodes: readonly HTMLElement[]): void {
  const wanted = new Set<Node>(nodes);
  for (const child of Array.from(grid.children)) {
    if (!wanted.has(child)) child.remove();
  }
  nodes.forEach((node, index) => {
    const at = grid.children[index];
    if (at !== node) grid.insertBefore(node, at ?? null);
  });
}

/**
 * Keyed update of an existing grid. A tile whose tip, size, colour and
 * next-up / overdue state are unchanged keeps its DOM node (only the
 * countdown text is refreshed), so focus, hover and running animations
 * survive the 30 s tick. Anything else gets a fresh node. Nodes are moved to
 * follow the board order. Returns the tiles that were newly built: the only
 * ones whose titles need fitting.
 */
export function updateBoard(grid: HTMLElement, options: BoardOptions): HTMLElement[] {
  const { state, now, onOpen, onMenu, onSetup, seen, resolveImage, questionsOnly = false } = options;
  const onAdd = questionsOnly ? undefined : options.onAdd;
  const onReorder = questionsOnly ? undefined : options.onReorder;
  const columns = clampColumns(state.settings.columns);
  const highlightWords = state.settings.highlight_words ?? [];
  grid.style.setProperty('--cols', String(columns));
  const shared = gridState(grid);
  const existingAdd = Array.from(grid.children).find((c): c is HTMLButtonElement => c.classList.contains('tile-add'));
  const plus = (): HTMLButtonElement | null => (onAdd ? existingAdd ?? addTile(onAdd) : null);

  const tips = visibleTips(state, now, questionsOnly);
  if (tips.length === 0) {
    shared.ranked = [];
    shared.onReorder = onReorder;
    const add = isReady(state) ? plus() : null;
    const empty = emptyMessage(state, onSetup, questionsOnly);
    arrange(grid, add ? [empty, add] : [empty]);
    return [];
  }

  const existing = new Map<string, HTMLElement>();
  for (const child of Array.from(grid.children)) {
    if (child instanceof HTMLElement && child.classList.contains('tile') && child.dataset.id) existing.set(child.dataset.id, child);
  }
  const built: HTMLElement[] = [];
  let entering = 0;
  const tiles = tips.map((tip) => {
    const isNextUp = tip.id === state.next_up;
    const kept = existing.get(tip.id);
    if (kept && updateTile(kept, tip, isNextUp, now, columns, highlightWords)) {
      seen?.add(tip.id);
      return kept;
    }
    const fresh = !seen || !seen.has(tip.id);
    const enterIndex = fresh ? entering++ : undefined;
    seen?.add(tip.id);
    const tile = renderTile({ tip, isNextUp, now, columns, onOpen, onMenu, enterIndex, resolveImage, highlightWords });
    built.push(tile);
    return tile;
  });
  const add = plus();
  arrange(grid, add ? [...tiles, add] : tiles);
  if (built.length > 0) {
    // Marks were computed against the old nodes; the next dragover redraws them.
    clearDragMarks(grid);
    const dragging = shared.dragged ? built.find((tile) => tile.dataset.id === shared.dragged) : undefined;
    dragging?.classList.add('tile-dragging');
  }

  // Keys come from the backend ranking (before manual order is applied).
  const shown = new Set(tips.map((tip) => tip.id));
  shared.ranked = state.tips.filter((tip) => shown.has(tip.id));
  shared.onReorder = onReorder;
  if (onReorder) wireDragAndDrop(grid);
  return built;
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

/** Columns the grid is laid out with (`--cols`, set by `updateBoard`). */
export function gridColumns(grid: HTMLElement): number {
  return clampColumns(Number.parseInt(grid.style.getPropertyValue('--cols'), 10));
}

const STEP: Record<string, (cols: number) => number> = {
  ArrowLeft: () => -1,
  ArrowRight: () => 1,
  ArrowUp: (cols) => -cols,
  ArrowDown: (cols) => cols,
};

/**
 * Arrow keys on the board. Left / Right step through the tiles in board
 * order (the "+" tile included); Up / Down jump by the column count. Both
 * stop at the ends rather than wrapping. Alt+Arrow on a focused tile moves
 * the tile itself by the same step and persists it through `onReorder`.
 * Only a focused tile (or the "+" tile) takes the arrows over; otherwise they
 * scroll the page as usual.
 */
export function handleBoardKeys(grid: HTMLElement, event: KeyboardEvent): void {
  const step = STEP[event.key];
  if (!step) return;
  const tiles = Array.from(grid.children).filter((c): c is HTMLButtonElement => c instanceof HTMLButtonElement && c.classList.contains('tile'));
  const index = tiles.findIndex((tile) => tile === document.activeElement);
  if (index < 0) return;
  event.preventDefault();
  const delta = step(gridColumns(grid));
  if (event.altKey) {
    moveTile(grid, tiles[index], delta);
    return;
  }
  tiles[Math.max(0, Math.min(tiles.length - 1, index + delta))]?.focus();
}

/** Alt+Arrow: move a tile `delta` places in the displayed order. */
function moveTile(grid: HTMLElement, tile: HTMLElement | undefined, delta: number): void {
  const id = tile?.dataset.id;
  const state = gridStates.get(grid);
  if (!id || !state?.onReorder) return;
  const shown = applyManualOrder(state.ranked);
  const from = shown.findIndex((tip) => tip.id === id);
  if (from < 0) return;
  // `orderForMove` takes the index among the *other* tiles, which for a
  // move to display position `to` is `to` itself.
  const to = Math.max(0, Math.min(shown.length - 1, from + delta));
  if (to === from) return;
  const order = orderForMove(state.ranked, id, to);
  if (order !== undefined) commitMove(state, id, order);
}
