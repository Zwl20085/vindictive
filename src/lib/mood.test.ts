import { describe, expect, it } from 'vitest';
import type { Tip } from '../types';
import { clawdFeeling, clawdMood, completedIds, isClawdMood, moodCounts, MAX_CELEBRATED_AT_ONCE, shouldCelebrate } from './mood';
import { DAY_MS, HOUR_MS, toNaive } from './time';

const now = new Date(2026, 9, 2, 14, 0, 0);
const at = (ms: number): string => toNaive(new Date(now.getTime() + ms));

function tip(id: string, patch: Partial<Tip> = {}): Tip {
  return { id, path: `${id}.md`, title: id, kind: 'task', priority: 'normal', status: 'open', body: '', remind_at: [], ...patch };
}

const overdue = (id: string): Tip => tip(id, { due_at: at(-HOUR_MS) });
const done = (id: string, patch: Partial<Tip> = {}): Tip => tip(id, { status: 'done', done_at: at(0), ...patch });

describe('moodCounts', () => {
  it('counts open, visible tips and the overdue ones among them', () => {
    const tips = [
      overdue('late'),
      overdue('late-but-snoozed'),
      tip('later', { due_at: at(3 * DAY_MS) }),
      done('finished', { due_at: at(-DAY_MS) }),
    ].map((t) => (t.id === 'late-but-snoozed' ? { ...t, snoozed_until_at: at(HOUR_MS) } : t));
    expect(moodCounts(tips, now)).toEqual({ open: 2, overdue: 1 });
  });

  it('treats an expired snooze as visible again', () => {
    expect(moodCounts([{ ...overdue('a'), snoozed_until_at: at(-1) }], now)).toEqual({ open: 1, overdue: 1 });
  });
});

describe('clawdMood', () => {
  const day = now;
  const night = new Date(2026, 9, 2, 23, 30);

  it('walks by default', () => {
    expect(clawdMood({ tips: [tip('a')], now: day, celebrating: false })).toBe('walking');
  });

  it('worries about overdue tips', () => {
    expect(clawdMood({ tips: [tip('a'), overdue('b')], now: day, celebrating: false })).toBe('worried');
  });

  it('relaxes when nothing open is visible', () => {
    expect(clawdMood({ tips: [], now: day, celebrating: false })).toBe('relaxed');
    expect(clawdMood({ tips: [done('a'), { ...tip('b'), snoozed_until_at: at(HOUR_MS) }], now: day, celebrating: false })).toBe('relaxed');
  });

  it('does not relax before the board has loaded', () => {
    expect(clawdMood({ tips: undefined, now: day, celebrating: false })).toBe('walking');
  });

  it('follows the priority celebrating > sleeping > worried > relaxed > walking', () => {
    expect(clawdMood({ tips: [overdue('a')], now: night, celebrating: true })).toBe('celebrating');
    expect(clawdMood({ tips: [overdue('a')], now: night, celebrating: false })).toBe('sleeping');
    expect(clawdMood({ tips: [], now: night, celebrating: false })).toBe('sleeping');
  });
});

describe('completedIds', () => {
  it('finds tips that went from open to done', () => {
    expect(completedIds([tip('a'), tip('b')], [done('a'), tip('b')])).toEqual(['a']);
  });

  it('counts a recurring tip that rolled forward with a new done_at', () => {
    const before = tip('weekly', { due_at: at(-HOUR_MS), done_at: at(-7 * DAY_MS) });
    const after = tip('weekly', { due_at: at(7 * DAY_MS), done_at: at(0) });
    expect(completedIds([before], [after])).toEqual(['weekly']);
  });

  it('ignores new, removed, reopened and already-done tips', () => {
    expect(completedIds([done('a'), tip('gone')], [done('a'), done('new'), tip('a2')])).toEqual([]);
    expect(completedIds([done('a')], [tip('a')])).toEqual([]);
  });
});

describe('shouldCelebrate', () => {
  it('never celebrates the first board', () => {
    expect(shouldCelebrate(undefined, [done('a')])).toBe(false);
  });

  it('celebrates a single completion', () => {
    expect(shouldCelebrate([tip('a')], [done('a')])).toBe(true);
    expect(shouldCelebrate([tip('a')], [tip('a')])).toBe(false);
  });

  it('stays quiet when many tips flip at once (a folder reload)', () => {
    const ids = Array.from({ length: MAX_CELEBRATED_AT_ONCE + 1 }, (_, i) => `t${i}`);
    expect(shouldCelebrate(ids.map((id) => tip(id)), ids.map((id) => done(id)))).toBe(false);
    const few = ids.slice(0, MAX_CELEBRATED_AT_ONCE);
    expect(shouldCelebrate(few.map((id) => tip(id)), few.map((id) => done(id)))).toBe(true);
  });
});

describe('isClawdMood', () => {
  it('accepts only known moods', () => {
    expect(isClawdMood('worried')).toBe(true);
    expect(isClawdMood('angry')).toBe(false);
    expect(isClawdMood(null)).toBe(false);
  });
});

describe('clawdFeeling', () => {
  it('says what Clawd feels in either language', () => {
    expect(clawdFeeling('worried', { open: 3, overdue: 2 }, 'en')).toBe('Clawd · 2 overdue');
    expect(clawdFeeling('worried', { open: 3, overdue: 2 }, 'zh')).toBe('Clawd · 2 项逾期');
    expect(clawdFeeling('relaxed', { open: 0, overdue: 0 }, 'en')).toBe('Clawd · all done');
    expect(clawdFeeling('sleeping', { open: 1, overdue: 1 }, 'en')).toBe('Clawd · zzz');
    expect(clawdFeeling('walking', { open: 4, overdue: 0 }, 'en')).toBe('Clawd · 4 to do');
    expect(clawdFeeling('walking', { open: 4, overdue: 0 }, 'zh')).toBe('Clawd · 4 项待办');
    expect(clawdFeeling('celebrating', { open: 4, overdue: 0 }, 'zh')).toBe('Clawd · 完成一项！');
  });
});
