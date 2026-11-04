import { describe, expect, it } from 'vitest';
import type { Tip } from '../types';
import {
  contrastRatio,
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
      fg: foregroundFor(DEADLINE_COLORS.soon),
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
    // Mid blue: 4.9:1 with dark text against 3.1:1 with light, so dark text now (was light under the 0.5 luminance cut).
    expect(foregroundFor('#2D89EF')).toBe(DARK_FG);
    expect(foregroundFor(KIND_COLORS.reading)).toBe(LIGHT_FG);
    expect(foregroundFor('nope')).toBe(LIGHT_FG);
  });
});

describe('text contrast', () => {
  it('computes WCAG contrast ratios', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21);
    expect(contrastRatio('#ffffff', '#000000')).toBeCloseTo(21);
    expect(contrastRatio('#777777', '#777777')).toBeCloseTo(1);
  });
  it('puts dark text on the amber deadline colour', () => {
    const amber = DEADLINE_COLORS.soon;
    expect(amber).toBe('#A87B1F');
    expect(contrastRatio(amber, LIGHT_FG)).toBeLessThan(4.5);
    expect(contrastRatio(amber, DARK_FG)).toBeGreaterThan(4.5);
    expect(foregroundFor(amber)).toBe(DARK_FG);
    expect(tileColor(tip({ kind: 'deadline', due_at: '2026-10-05T00:00:00' }), now).fg).toBe(DARK_FG);
  });
  it('keeps light text on every other palette colour', () => {
    for (const bg of [...Object.values(KIND_COLORS), DEADLINE_COLORS.later, DEADLINE_COLORS.critical, OVERDUE_COLOR, DONE_COLOR]) {
      expect(foregroundFor(bg), bg).toBe(LIGHT_FG);
    }
  });
  it('always picks the higher-contrast option', () => {
    for (const bg of ['#FF0097', '#2D89EF', '#FFC40D', '#00A300', '#603CBA', '#888', '#fff', '#000']) {
      const fg = foregroundFor(bg);
      const other = fg === DARK_FG ? LIGHT_FG : DARK_FG;
      expect(contrastRatio(bg, fg), bg).toBeGreaterThanOrEqual(contrastRatio(bg, other));
    }
  });
});
