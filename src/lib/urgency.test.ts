import { describe, expect, it } from 'vitest';
import { urgencyOf, urgencyOfNaive } from './urgency';
import { DAY_MS, HOUR_MS } from './time';

const now = new Date(2026, 9, 1, 12, 0, 0);
const at = (ms: number): Date => new Date(now.getTime() + ms);

describe('urgencyOf', () => {
  it('classifies like the Rust side', () => {
    expect(urgencyOf(undefined, now)).toBe('none');
    expect(urgencyOf(at(20 * DAY_MS), now)).toBe('later');
    expect(urgencyOf(at(7 * DAY_MS), now)).toBe('soon');
    expect(urgencyOf(at(7 * DAY_MS + 1), now)).toBe('later');
    expect(urgencyOf(at(48 * HOUR_MS), now)).toBe('critical');
    expect(urgencyOf(at(0), now)).toBe('overdue');
    expect(urgencyOf(at(-DAY_MS), now)).toBe('overdue');
  });
  it('accepts naive strings', () => {
    expect(urgencyOfNaive('2026-10-01T13:00:00', now)).toBe('critical');
    expect(urgencyOfNaive(undefined, now)).toBe('none');
  });
});
