import { afterEach, describe, expect, it, vi } from 'vitest';
import { SAMPLE_SETTINGS } from '../mock/sample-tips';
import type { BoardState, Tip } from '../types';
import { createBoard, handleBoardKeys, renderBoard, updateBoard, type BoardOptions } from './board';

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
    press(grid, 'ArrowRight');
    expect(focusedId()).toBe('0');
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
    press(grid, 'ArrowLeft', true);
    expect(onReorder).toHaveBeenLastCalledWith('1', -1);
    press(grid, 'ArrowDown', true);
    expect(onReorder).toHaveBeenLastCalledWith('1', 5);
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
