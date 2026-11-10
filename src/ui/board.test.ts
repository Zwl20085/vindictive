import { afterEach, describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import type { BoardState, Tip } from '../types';
import { createBoard, handleBoardKeys, renderBoard, updateBoard, visibleTips, type BoardOptions } from './board';

const NOW = new Date('2026-11-04T10:00:00');

function tip(id: string, extra: Partial<Tip> = {}): Tip {
  return { id, path: `${id}.md`, sha: `sha-${id}`, title: `Tip ${id}`, kind: 'task', priority: 'normal', status: 'open', body: '', remind_at: [], ...extra };
}

function board(tips: Tip[], extra: Partial<BoardState> = {}): BoardState {
  return { tips, settings: { ...SAMPLE_SETTINGS, columns: 3, show_done: false }, now: '2026-11-04T10:00:00', ...extra };
}

function options(state: BoardState, extra: Partial<BoardOptions> = {}): BoardOptions {
  return { state, now: NOW, onOpen: vi.fn(), seen: new Set<string>(), ...extra };
}

function ids(grid: HTMLElement): string[] {
  return Array.from(grid.querySelectorAll<HTMLElement>('.tile[data-id]')).map((t) => t.dataset.id ?? '');
}

function tileFor(grid: HTMLElement, id: string): HTMLElement {
  const node = Array.from(grid.querySelectorAll<HTMLElement>('.tile[data-id]')).find((t) => t.dataset.id === id);
  if (!node) throw new Error(`no tile ${id}`);
  return node;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('keyed board update', () => {
  it('keeps the node of an unchanged tip and builds nothing new', () => {
    const grid = createBoard();
    const opts = options(board([tip('a'), tip('b')]));
    expect(updateBoard(grid, opts)).toHaveLength(2);
    const a = tileFor(grid, 'a');
    const built = updateBoard(grid, { ...opts, state: board([tip('a'), tip('b')]) });
    expect(built).toHaveLength(0);
    expect(tileFor(grid, 'a')).toBe(a);
  });

  it('replaces a tile whose sha changed', () => {
    const grid = createBoard();
    const opts = options(board([tip('a'), tip('b')]));
    updateBoard(grid, opts);
    const a = tileFor(grid, 'a');
    const b = tileFor(grid, 'b');
    const built = updateBoard(grid, { ...opts, state: board([tip('a', { sha: 'sha-a2', title: 'Edited' }), tip('b')]) });
    expect(built).toHaveLength(1);
    expect(tileFor(grid, 'a')).not.toBe(a);
    expect(tileFor(grid, 'a').textContent).toContain('Edited');
    expect(tileFor(grid, 'b')).toBe(b);
  });

  it('moves kept nodes to follow a new order', () => {
    const grid = createBoard();
    const opts = options(board([tip('a'), tip('b'), tip('c')]));
    updateBoard(grid, opts);
    const nodes = ['a', 'b', 'c'].map((id) => tileFor(grid, id));
    updateBoard(grid, { ...opts, state: board([tip('c'), tip('a'), tip('b')]) });
    expect(ids(grid)).toEqual(['c', 'a', 'b']);
    expect(tileFor(grid, 'c')).toBe(nodes[2]);
    expect(tileFor(grid, 'a')).toBe(nodes[0]);
  });

  it('removes tiles that left the board and keeps the "+" tile last', () => {
    const grid = createBoard();
    const opts = options(board([tip('a'), tip('b')]), { onAdd: vi.fn() });
    updateBoard(grid, opts);
    const plus = grid.querySelector('.tile-add');
    updateBoard(grid, { ...opts, state: board([tip('b')]) });
    expect(ids(grid)).toEqual(['b']);
    expect(grid.lastElementChild).toBe(plus);
  });

  it('refreshes the countdown of a kept tile', () => {
    const grid = createBoard();
    const due = tip('a', { kind: 'deadline', due_at: '2026-11-06T10:00:00' });
    const opts = options(board([due]));
    updateBoard(grid, opts);
    const node = tileFor(grid, 'a');
    const before = node.querySelector('.tile-countdown')?.textContent;
    updateBoard(grid, { ...opts, now: new Date('2026-11-04T13:00:00') });
    expect(tileFor(grid, 'a')).toBe(node);
    expect(node.querySelector('.tile-countdown')?.textContent).not.toBe(before);
    expect(node.getAttribute('aria-label')).toContain(node.querySelector('.tile-countdown')?.textContent ?? '?');
  });

  it('plays the entry animation only for unseen ids', () => {
    const grid = createBoard();
    const seen = new Set<string>();
    updateBoard(grid, options(board([tip('a')]), { seen }));
    expect(tileFor(grid, 'a').classList.contains('tile-enter')).toBe(true);
    updateBoard(grid, options(board([tip('a', { sha: 'x' }), tip('b')]), { seen }));
    expect(tileFor(grid, 'a').classList.contains('tile-enter')).toBe(false);
    expect(tileFor(grid, 'b').classList.contains('tile-enter')).toBe(true);
  });

  it('wires drag and drop once, with the latest ranking', () => {
    const grid = createBoard();
    const add = vi.spyOn(grid, 'addEventListener');
    const onReorder = vi.fn();
    updateBoard(grid, options(board([tip('a'), tip('b')]), { onReorder }));
    const wired = add.mock.calls.length;
    updateBoard(grid, options(board([tip('a'), tip('b'), tip('c')]), { onReorder }));
    expect(add.mock.calls.length).toBe(wired);
  });

  it('uses a group role with a label instead of list', () => {
    const grid = renderBoard(options(board([tip('a')])));
    expect(grid.getAttribute('role')).toBe('group');
    expect(grid.getAttribute('aria-label')).toBeTruthy();
  });

  it('adds lazy, async decoding to thumbnails', () => {
    const resolveImage = vi.fn(async () => 'data:image/png;base64,');
    const grid = renderBoard(options(board([tip('a', { priority: 'high', images: ['fig/a.png'] })]), { resolveImage }));
    const img = grid.querySelector('img');
    expect(img?.getAttribute('loading')).toBe('lazy');
    expect(img?.getAttribute('decoding')).toBe('async');
    expect(resolveImage).toHaveBeenCalledWith('fig/a.png', 'sha-a');
  });
});

describe('board keys', () => {
  function setup(n: number, extra: Partial<BoardOptions> = {}): HTMLElement {
    const grid = renderBoard(options(board(Array.from({ length: n }, (_, i) => tip(String(i)))), { onAdd: vi.fn(), ...extra }));
    document.body.append(grid);
    return grid;
  }
  const press = (grid: HTMLElement, key: string, altKey = false): void => handleBoardKeys(grid, new KeyboardEvent('keydown', { key, altKey, cancelable: true }));
  const focusedId = (): string | undefined => (document.activeElement as HTMLElement | null)?.dataset.id;

  it('Left / Right step linearly and stop at the ends', () => {
    const grid = setup(4);
    tileFor(grid, '0').focus();
    press(grid, 'ArrowLeft');
    expect(focusedId()).toBe('0');
    press(grid, 'ArrowRight');
    expect(focusedId()).toBe('1');
  });

  it('Up / Down jump by the column count', () => {
    const grid = setup(7);
    tileFor(grid, '1').focus();
    press(grid, 'ArrowDown');
    expect(focusedId()).toBe('4');
    press(grid, 'ArrowUp');
    expect(focusedId()).toBe('1');
  });

  it('reaches the "+" tile', () => {
    const grid = setup(2);
    tileFor(grid, '1').focus();
    press(grid, 'ArrowRight');
    expect(document.activeElement?.classList.contains('tile-add')).toBe(true);
  });

  it('Alt+Arrow moves the focused tile through onReorder', () => {
    const onReorder = vi.fn();
    const grid = setup(5, { onReorder });
    tileFor(grid, '1').focus();
    press(grid, 'ArrowRight', true);
    expect(onReorder).toHaveBeenCalledWith('1', 2.5);
    // No board update in between: each move starts where the last one put the tile.
    press(grid, 'ArrowLeft', true);
    expect(onReorder).toHaveBeenLastCalledWith('1', 1);
    press(grid, 'ArrowDown', true);
    expect(onReorder).toHaveBeenLastCalledWith('1', 5);
    expect(focusedId()).toBe('1');
  });

  it('two quick Alt+Arrow moves both count, before the backend answers', () => {
    const onReorder = vi.fn();
    const grid = setup(5, { onReorder });
    tileFor(grid, '1').focus();
    press(grid, 'ArrowRight', true);
    press(grid, 'ArrowRight', true);
    // 0 1 2 3 4 -> 0 2 1 3 4 (between 2 and 3) -> 0 2 3 1 4 (between 3 and 4).
    expect(onReorder.mock.calls).toEqual([
      ['1', 2.5],
      ['1', 3.5],
    ]);
  });

  it('leaves arrow keys alone (page scroll) when no tile has focus', () => {
    const grid = setup(3);
    const outside = document.createElement('input');
    document.body.append(outside);
    outside.focus();
    const event = new KeyboardEvent('keydown', { key: 'ArrowDown', cancelable: true });
    handleBoardKeys(grid, event);
    expect(event.defaultPrevented).toBe(false);
    expect(document.activeElement).toBe(outside);
    (document.activeElement as HTMLElement).blur();
    const again = new KeyboardEvent('keydown', { key: 'ArrowRight', cancelable: true });
    handleBoardKeys(grid, again);
    expect(again.defaultPrevented).toBe(false);
    expect(document.activeElement).toBe(document.body);
  });

  it('the "+" tile takes the arrows too', () => {
    const grid = setup(2);
    (grid.querySelector('.tile-add') as HTMLElement).focus();
    const event = new KeyboardEvent('keydown', { key: 'ArrowLeft', cancelable: true });
    handleBoardKeys(grid, event);
    expect(event.defaultPrevented).toBe(true);
    expect(focusedId()).toBe('1');
  });


  it('Alt+Arrow at the edge does nothing', () => {
    const onReorder = vi.fn();
    const grid = setup(3, { onReorder });
    tileFor(grid, '0').focus();
    press(grid, 'ArrowLeft', true);
    press(grid, 'ArrowUp', true);
    expect(onReorder).not.toHaveBeenCalled();
  });
});

describe('parse warnings on tiles', () => {
  it('marks a tile with warnings and names them in its label', () => {
    const grid = renderBoard(options(board([tip('a', { warnings: ['due: cannot parse "15/10/2026"'] }), tip('b')])));
    const a = tileFor(grid, 'a');
    expect(a.dataset.warn).toBe('1');
    expect(a.querySelector('.tile-warn')?.textContent).toBe('!');
    expect(a.getAttribute('aria-label')).toContain('parse warnings');
    expect(tileFor(grid, 'b').hasAttribute('data-warn')).toBe(false);
    expect(tileFor(grid, 'b').querySelector('.tile-warn')).toBeNull();
  });

  it('rebuilds the tile when its warnings change', () => {
    const grid = createBoard();
    const opts = options(board([tip('a')]));
    updateBoard(grid, opts);
    const before = tileFor(grid, 'a');
    updateBoard(grid, { ...opts, state: board([tip('a', { warnings: ['remind: "-1d" needs a due date'] })]) });
    expect(tileFor(grid, 'a')).not.toBe(before);
    expect(tileFor(grid, 'a').dataset.warn).toBe('1');
    const warned = tileFor(grid, 'a');
    updateBoard(grid, { ...opts, state: board([tip('a')]) });
    expect(tileFor(grid, 'a')).not.toBe(warned);
    expect(tileFor(grid, 'a').hasAttribute('data-warn')).toBe(false);
  });
});

describe('highlights on tiles', () => {
  const withWords = (tips: Tip[], words: string[]): BoardState => {
    const state = board(tips);
    return { ...state, settings: { ...state.settings, highlight_words: words } };
  };

  it('marks the tip and global keywords in the title, with no automatic patterns', () => {
    const grid = renderBoard(options(withWords([tip('a', { title: 'TODO thesis draft 2026-11-04', highlight: ['draft'] })], ['Thesis'])));
    const title = tileFor(grid, 'a').querySelector('.tile-title');
    expect(Array.from(title?.querySelectorAll('mark') ?? []).map((m) => `${m.className}|${m.textContent}`)).toEqual(['hl hl-kw|thesis', 'hl hl-kw|draft']);
    expect(title?.textContent).toBe('TODO thesis draft 2026-11-04');
  });

  it('rebuilds a tile when the global words change, and keeps it otherwise', () => {
    const grid = createBoard();
    const opts = options(withWords([tip('a', { title: 'thesis' })], []));
    updateBoard(grid, opts);
    const before = tileFor(grid, 'a');
    expect(updateBoard(grid, { ...opts, state: withWords([tip('a', { title: 'thesis' })], []) })).toHaveLength(0);
    expect(updateBoard(grid, { ...opts, state: withWords([tip('a', { title: 'thesis' })], ['thesis']) })).toHaveLength(1);
    expect(tileFor(grid, 'a')).not.toBe(before);
    expect(tileFor(grid, 'a').querySelector('mark.hl-kw')?.textContent).toBe('thesis');
  });
});

describe('drag and drop', () => {
  const drag = (target: EventTarget, type: string): Event => {
    const event = new Event(type, { bubbles: true, cancelable: true });
    target.dispatchEvent(event);
    return event;
  };

  it('a tile replaced mid-drag does not leave the drag running', () => {
    const onReorder = vi.fn();
    const grid = createBoard();
    document.body.append(grid);
    const opts = options(board([tip('a'), tip('b'), tip('c')]), { onReorder });
    updateBoard(grid, opts);
    const a = tileFor(grid, 'a');
    drag(a, 'dragstart');
    expect(a.classList.contains('tile-dragging')).toBe(true);
    drag(tileFor(grid, 'b'), 'dragover');
    // A rescan replaces tile a while it is being dragged: the new node carries the drag look.
    updateBoard(grid, { ...opts, state: board([tip('a', { sha: 'sha-a2' }), tip('b'), tip('c')]) });
    expect(tileFor(grid, 'a')).not.toBe(a);
    expect(tileFor(grid, 'a').classList.contains('tile-dragging')).toBe(true);
    expect(grid.querySelector('.tile-drop-before, .tile-drop-after')).toBeNull();
    // Its dragend fires on the detached node: the drag still ends.
    drag(a, 'dragend');
    expect(grid.querySelector('.tile-dragging')).toBeNull();
    // A later drop on the grid is no longer taken for a move.
    drag(grid, 'drop');
    expect(onReorder).not.toHaveBeenCalled();
  });

  it('a drop anywhere in the document ends the drag', () => {
    const onReorder = vi.fn();
    const grid = createBoard();
    document.body.append(grid);
    updateBoard(grid, options(board([tip('a'), tip('b')]), { onReorder }));
    drag(tileFor(grid, 'a'), 'dragstart');
    drag(document.body, 'drop');
    expect(grid.querySelector('.tile-dragging')).toBeNull();
    const over = drag(tileFor(grid, 'b'), 'dragover');
    expect(over.defaultPrevented).toBe(false);
  });

  it('a drop applies the new order locally, so a quick Alt+Arrow builds on it', () => {
    const onReorder = vi.fn();
    const grid = createBoard();
    document.body.append(grid);
    updateBoard(grid, options(board([tip('a'), tip('b'), tip('c')]), { onReorder, onAdd: vi.fn() }));
    drag(tileFor(grid, 'a'), 'dragstart');
    drag(grid, 'drop'); // empty grid: a goes last
    expect(onReorder).toHaveBeenLastCalledWith('a', 3);
    tileFor(grid, 'a').focus();
    handleBoardKeys(grid, new KeyboardEvent('keydown', { key: 'ArrowLeft', altKey: true, cancelable: true }));
    // b c a -> b a c: between b (1) and c (2).
    expect(onReorder).toHaveBeenLastCalledWith('a', 1.5);
  });
});

describe('questions filter', () => {
  const q = (id: string, extra: Partial<Tip> = {}): Tip => tip(`questions/${id}`, { path: `questions/${id}.md`, ...extra });

  it('shows every tip without the filter and only question tips with it', () => {
    const state = board([tip('a'), q('a'), tip('b'), q('c', { status: 'done' })]);
    expect(visibleTips(state, NOW).map((t) => t.id)).toEqual(['a', 'questions/a', 'b']);
    expect(visibleTips(state, NOW, true).map((t) => t.id)).toEqual(['questions/a']);
    const withDone = board(state.tips, { settings: { ...state.settings, show_done: true } });
    expect(visibleTips(withDone, NOW, true).map((t) => t.id)).toEqual(['questions/a', 'questions/c']);
  });

  it('a question and a main tip of the same name are two tiles', () => {
    const grid = createBoard();
    updateBoard(grid, options(board([tip('a'), q('a')])));
    expect(ids(grid)).toEqual(['a', 'questions/a']);
  });

  it('offers no "+" tile and no reordering while filtered, and says when there is nothing', () => {
    const grid = createBoard();
    const onReorder = vi.fn();
    updateBoard(grid, options(board([tip('a'), q('b')]), { onAdd: vi.fn(), onReorder, questionsOnly: true }));
    expect(ids(grid)).toEqual(['questions/b']);
    expect(grid.querySelector('.tile-add')).toBeNull();
    document.body.append(grid);
    const only = tileFor(grid, 'questions/b');
    only.focus();
    handleBoardKeys(grid, new KeyboardEvent('keydown', { key: 'ArrowLeft', altKey: true }));
    expect(onReorder).not.toHaveBeenCalled();
    updateBoard(grid, options(board([tip('a')]), { onAdd: vi.fn(), questionsOnly: true }));
    expect(grid.querySelector('.tile-add')).toBeNull();
    expect(grid.querySelector('.board-empty')?.textContent).toBe('No questions waiting for an answer.');
  });
});
