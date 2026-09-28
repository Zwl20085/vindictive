import { describe, expect, it } from 'vitest';
import {
  countdown,
  formatClock,
  formatLocal,
  formatSpan,
  HOUR_MS,
  DAY_MS,
  MINUTE_MS,
  minutesUntilTomorrowMorning,
  parseNaive,
  toNaive,
  WEEK_MS,
} from './time';

const now = new Date(2026, 9, 1, 12, 0, 0);

describe('parseNaive', () => {
  it('parses full and short forms as local time', () => {
    expect(parseNaive('2026-10-15T23:59:00')).toEqual(new Date(2026, 9, 15, 23, 59, 0));
    expect(parseNaive('2026-10-15 08:30')).toEqual(new Date(2026, 9, 15, 8, 30, 0));
    expect(parseNaive('2026-10-15')).toEqual(new Date(2026, 9, 15, 0, 0, 0));
  });
  it('rejects junk', () => {
    expect(parseNaive('soon')).toBeUndefined();
    expect(parseNaive(undefined)).toBeUndefined();
    expect(parseNaive('')).toBeUndefined();
  });
});

describe('formatSpan', () => {
  it('picks the right magnitude', () => {
    expect(formatSpan(3 * WEEK_MS + DAY_MS)).toBe('3w');
    expect(formatSpan(5 * DAY_MS)).toBe('5d');
    expect(formatSpan(2 * DAY_MS + 4 * HOUR_MS)).toBe('2d 4h');
    expect(formatSpan(2 * DAY_MS)).toBe('2d');
    expect(formatSpan(6 * HOUR_MS + 10 * MINUTE_MS)).toBe('6h');
    expect(formatSpan(45 * MINUTE_MS)).toBe('45m');
    expect(formatSpan(10)).toBe('1m');
  });
});

describe('countdown', () => {
  it('formats future and overdue', () => {
    expect(countdown(new Date(now.getTime() + 2 * DAY_MS + 4 * HOUR_MS), now)).toBe('2d 4h');
    expect(countdown(new Date(now.getTime() - 2 * HOUR_MS), now)).toBe('overdue 2h');
    expect(countdown(now, now)).toBe('overdue 1m');
  });
});

describe('minutesUntilTomorrowMorning', () => {
  it('targets 09:00 next day', () => {
    expect(minutesUntilTomorrowMorning(now)).toBe(21 * 60);
    expect(minutesUntilTomorrowMorning(new Date(2026, 9, 1, 23, 30))).toBe(9 * 60 + 30);
  });
});

describe('formatting', () => {
  it('formats dates and clocks', () => {
    expect(formatLocal(new Date(2026, 9, 15, 23, 59))).toBe('Oct 15, 23:59');
    expect(formatClock(new Date(2026, 0, 1, 7, 5))).toBe('07:05');
    expect(toNaive(new Date(2026, 9, 15, 8, 3, 9))).toBe('2026-10-15T08:03:09');
  });
});
