import { describe, expect, it } from 'vitest';
import { toNaive } from '../lib/time';
import { mockInvoke, nextOccurrence, parseCaptureText } from './backend';

const now = new Date(2026, 9, 14, 12, 0, 0); // Wed 14 Oct 2026

describe('mock capture parsing', () => {
  it('reads priority, kind, tags, place and a date with an optional time', () => {
    const tip = parseCaptureText('Check coil !high #lab >deadline @tomorrow 9:30 ^Lab302', now);
    expect(tip).toMatchObject({
      title: 'Check coil',
      priority: 'high',
      kind: 'deadline',
      tags: ['lab'],
      location: 'Lab302',
      due: '2026-10-15 09:30',
      due_at: '2026-10-15T09:30:00',
    });
  });
  it('a bare date is the end of that day; full-width markers count', () => {
    const tip = parseCaptureText('Submit ＠2026-11-02 ！low', now);
    expect(tip).toMatchObject({ title: 'Submit', priority: 'low', due: '2026-11-02', due_at: '2026-11-02T23:59:00' });
  });
  it('keeps unknown @words in the title and needs a title', () => {
    expect(parseCaptureText('ping @alice', now).title).toBe('ping @alice');
    expect(parseCaptureText('ping @alice', now).due).toBeUndefined();
    expect(() => parseCaptureText('#only !high', now)).toThrow(/no title/);
  });
});

describe('mock create_tip', () => {
  it('never reuses an id', async () => {
    const text = 'Unique id probe';
    await mockInvoke('create_tip', { text });
    await mockInvoke('create_tip', { text });
    const state = await mockInvoke('create_tip', { text });
    const ids = state.tips.filter((t) => t.title === text).map((t) => t.id);
    expect(new Set(ids).size).toBe(3);
    const base = ids.find((id) => !/-\d$/.test(id));
    expect(base).toBeDefined();
    expect(ids.sort()).toEqual([base, `${base}-2`, `${base}-3`].sort());
  });
});

describe('mock mark_done', () => {
  it('rolls a repeating tip forward instead of closing it', async () => {
    // The sample "Group meeting" repeats weekly.
    const before = (await mockInvoke('get_state')).tips.find((t) => t.repeat?.startsWith('weekly'));
    if (!before?.due_at) throw new Error('sample has no weekly tip with a due time');
    const after = await mockInvoke('mark_done', { id: before.id });
    const rolled = after.tips.find((t) => t.id === before.id);
    const expected = new Date(new Date(before.due_at).getTime());
    expected.setDate(expected.getDate() + 7);
    expect(rolled?.status).toBe('open');
    expect(rolled?.due_at).toBe(toNaive(expected));
    expect(rolled?.due).toBe(toNaive(expected).slice(0, 16).replace('T', ' '));
  });
  it('closes a one-off tip', async () => {
    const created = await mockInvoke('create_tip', { text: 'One-off thing' });
    const tip = created.tips.find((t) => t.title === 'One-off thing');
    const after = await mockInvoke('mark_done', { id: tip?.id ?? '' });
    expect(after.tips.find((t) => t.id === tip?.id)?.status).toBe('done');
  });
});

describe('nextOccurrence', () => {
  it('steps one interval', () => {
    const at = (d: Date | undefined): string | undefined => (d ? toNaive(d) : undefined);
    expect(at(nextOccurrence('daily', now))).toBe('2026-10-15T12:00:00');
    expect(at(nextOccurrence('weekly on mon', now))).toBe('2026-10-21T12:00:00');
    expect(at(nextOccurrence('monthly on 1', now))).toBe('2026-11-14T12:00:00');
    expect(at(nextOccurrence('yearly on 03-15', now))).toBe('2027-10-14T12:00:00');
    // Friday → Monday.
    expect(at(nextOccurrence('weekdays', new Date(2026, 9, 16, 9, 0)))).toBe('2026-10-19T09:00:00');
    expect(nextOccurrence('every blue moon', now)).toBeUndefined();
  });
});
