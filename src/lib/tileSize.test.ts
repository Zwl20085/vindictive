import { describe, expect, it } from 'vitest';
import type { Tip } from '../types';
import { spanFor, tileSize } from './tileSize';

const now = new Date(2026, 9, 1, 12, 0, 0);

function tip(over: Partial<Tip>): Tip {
  return {
    id: 'x',
    path: 'tips/x.md',
    title: 'x',
    kind: 'task',
    priority: 'normal',
    status: 'open',
    body: '',
    remind_at: [],
    ...over,
  };
}

describe('tileSize', () => {
  it('next-up is always wide', () => {
    expect(tileSize(tip({ priority: 'low' }), true, now)).toBe('wide');
  });
  it('high priority is md', () => {
    expect(tileSize(tip({ priority: 'high' }), false, now)).toBe('md');
  });
  it('deadline within a week is md, later is sm', () => {
    expect(tileSize(tip({ kind: 'deadline', due_at: '2026-10-05T23:59:00' }), false, now)).toBe('md');
    expect(tileSize(tip({ kind: 'deadline', due_at: '2026-09-30T23:59:00' }), false, now)).toBe('md');
    expect(tileSize(tip({ kind: 'deadline', due_at: '2026-12-01T23:59:00' }), false, now)).toBe('sm');
  });
  it('non-deadline tasks due soon stay sm', () => {
    expect(tileSize(tip({ kind: 'task', due_at: '2026-10-02T09:00:00' }), false, now)).toBe('sm');
  });
});

describe('spanFor', () => {
  it('clamps to available columns', () => {
    expect(spanFor('wide', 4)).toEqual([4, 2]);
    expect(spanFor('wide', 2)).toEqual([2, 2]);
    expect(spanFor('md', 1)).toEqual([1, 2]);
    expect(spanFor('sm', 6)).toEqual([1, 1]);
  });
});
