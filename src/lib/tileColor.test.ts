import { describe, expect, it } from 'vitest';
import type { Tip } from '../types';
import {
  DARK_FG,
  DEADLINE_COLORS,
  DONE_COLOR,
  foregroundFor,
  isHexColor,
  KIND_COLORS,
  LIGHT_FG,
  luminance,
  OVERDUE_COLOR,
  tileColor,
} from './tileColor';

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

describe('tileColor', () => {
  it('uses kind colours for non-deadlines', () => {
    expect(tileColor(tip({ kind: 'task' }), now).bg).toBe(KIND_COLORS.task);
    expect(tileColor(tip({ kind: 'note' }), now).bg).toBe(KIND_COLORS.note);
    expect(tileColor(tip({ kind: 'reading' }), now).fg).toBe(LIGHT_FG);
  });
  it('escalates deadlines by urgency', () => {
    const d = (due: string): Tip => tip({ kind: 'deadline', due_at: due });
    expect(tileColor(d('2026-12-01T00:00:00'), now).bg).toBe(DEADLINE_COLORS.later);
    expect(tileColor(d('2026-10-05T00:00:00'), now)).toEqual({
      bg: DEADLINE_COLORS.soon,
      fg: DARK_FG,
      overdue: false,
    });
    expect(tileColor(d('2026-10-02T00:00:00'), now).bg).toBe(DEADLINE_COLORS.critical);
    expect(tileColor(d('2026-09-30T00:00:00'), now)).toEqual({
      bg: OVERDUE_COLOR,
      fg: LIGHT_FG,
      overdue: true,
    });
  });
  it('overdue applies to any kind', () => {
    expect(tileColor(tip({ kind: 'task', due_at: '2026-09-30T00:00:00' }), now).bg).toBe(OVERDUE_COLOR);
  });
  it('done is grey, override wins over kind', () => {
    expect(tileColor(tip({ status: 'done' }), now).bg).toBe(DONE_COLOR);
    expect(tileColor(tip({ color: '#FF0097' }), now).bg).toBe('#FF0097');
    expect(tileColor(tip({ color: '#fff' }), now).fg).toBe(DARK_FG);
    expect(tileColor(tip({ color: 'red' }), now).bg).toBe(KIND_COLORS.task);
  });
});

describe('colour helpers', () => {
  it('validates hex and computes luminance', () => {
    expect(isHexColor('#abc')).toBe(true);
    expect(isHexColor('#ABCDEF')).toBe(true);
    expect(isHexColor('blue')).toBe(false);
    expect(isHexColor(undefined)).toBe(false);
    expect(luminance('#000000')).toBeCloseTo(0);
    expect(luminance('#fff')).toBeCloseTo(1);
    expect(foregroundFor('#FFC40D')).toBe(DARK_FG);
    expect(foregroundFor('#2D89EF')).toBe(LIGHT_FG);
    expect(foregroundFor('nope')).toBe(LIGHT_FG);
  });
});
