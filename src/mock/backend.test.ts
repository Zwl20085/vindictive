import { describe, expect, it } from 'vitest';
import { toNaive } from '../lib/time';
import type { Tip, TipEdit } from '../types';
import { withAgentState } from '../lib/agent';
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

describe('mock create_tip_full', () => {
  const edit = (patch: Partial<TipEdit> = {}): TipEdit => ({
    title: 'Form-made tip',
    kind: 'event',
    priority: 'high',
    due: '2026-10-20 10:00',
    location: 'Room 4.12',
    tags: ['group'],
    remind: ['-1h', '2026-10-19 09:00'],
    repeat: 'weekly on tue',
    links: ['https://example.org'],
    images: ['figures/a.png'],
    color: '#4A3B6B',
    size: 'wide',
    highlight: ['agenda'],
    body: '',
    ...patch,
  });

  it('creates every form field, a template body when empty and a fresh id each time', async () => {
    const first = await mockInvoke('create_tip_full', { edit: edit() });
    const second = await mockInvoke('create_tip_full', { edit: edit({ body: 'My own notes' }) });
    const made = second.tips.filter((t) => t.title === 'Form-made tip');
    expect(new Set(made.map((t) => t.id)).size).toBe(2);
    const tip = first.tips.find((t) => t.title === 'Form-made tip');
    expect(tip).toMatchObject({
      kind: 'event',
      priority: 'high',
      status: 'open',
      due: '2026-10-20 10:00',
      due_at: '2026-10-20T10:00:00',
      location: 'Room 4.12',
      remind: ['-1h', '2026-10-19 09:00'],
      remind_at: ['2026-10-20T09:00:00', '2026-10-19T09:00:00'],
      repeat: 'weekly on tue',
      links: ['https://example.org'],
      images: ['figures/a.png'],
      color: '#4A3B6B',
      size: 'wide',
      highlight: ['agenda'],
    });
    expect(tip?.body).toMatch(/Agenda/);
    expect(made.find((t) => t.body === 'My own notes')).toBeDefined();
  });

  it('rejects what the backend would reject', async () => {
    await expect(mockInvoke('create_tip_full', { edit: edit({ title: '  ' }) })).rejects.toThrow(/title/);
    await expect(mockInvoke('create_tip_full', { edit: edit({ color: 'red' }) })).rejects.toThrow(/colour/);
    await expect(mockInvoke('create_tip_full', { edit: edit({ size: 'huge' }) })).rejects.toThrow(/size/);
    await expect(mockInvoke('create_tip_full', { edit: edit({ due: 'someday' }) })).rejects.toThrow(/not a date/);
  });

  it('update_tip writes the new fields too', async () => {
    const state = await mockInvoke('create_tip_full', { edit: edit({ title: 'To be edited' }) });
    const tip = state.tips.find((t) => t.title === 'To be edited');
    if (!tip) throw new Error('not created');
    const after = await mockInvoke('update_tip', { id: tip.id, base: tip.sha ?? null, edit: edit({ title: 'To be edited', highlight: [], color: null, size: null, links: [] }) });
    const edited = after.tips.find((t) => t.id === tip.id);
    expect(edited?.highlight).toBeUndefined();
    expect(edited?.color).toBeUndefined();
    expect(edited?.size).toBeUndefined();
    expect(edited?.links).toBeUndefined();
    expect(edited?.repeat).toBe('weekly on tue');
  });
});

describe('mock sample data', () => {
  it('shows highlight words in the settings and on two tips', async () => {
    const state = await mockInvoke('get_state');
    expect(state.settings.highlight_words.length).toBeGreaterThan(0);
    expect(state.tips.filter((t) => (t.highlight?.length ?? 0) > 0).length).toBeGreaterThanOrEqual(2);
  });
});

describe('mock agent tips', () => {
  const ID = 'agent-survey-hairpin';
  const agentTip = async (): Promise<Tip> => {
    const tip = (await mockInvoke('get_state')).tips.find((t) => t.id === ID);
    if (!tip) throw new Error('no sample agent tip');
    return tip;
  };
  /** The full Edit form for `tip`, every field as the file has it. */
  const formOf = (tip: Tip): TipEdit => ({
    title: tip.title,
    kind: tip.kind,
    priority: tip.priority,
    due: tip.due ?? null,
    location: tip.location ?? null,
    tags: tip.tags ?? [],
    remind: tip.remind ?? [],
    repeat: tip.repeat ?? null,
    links: tip.links ?? [],
    images: tip.images ?? [],
    color: tip.color ?? null,
    size: tip.size ?? null,
    highlight: tip.highlight ?? [],
    body: tip.body,
  });

  it('the sample is finished: its stored hash is the hash of its body', async () => {
    const tip = await agentTip();
    expect(tip.agent_state).toBe('done');
    expect(withAgentState(tip).agent_state).toBe('done');
  });

  it('every board write keeps the agent map; only a body edit re-queues', async () => {
    const original = await agentTip();
    const map = original.agent;
    const check = async (what: string, want: Tip['agent_state']) => {
      const tip = await agentTip();
      expect(tip.agent, what).toEqual(map);
      expect(tip.agent_state, what).toBe(want);
    };
    await mockInvoke('set_color', { id: ID, color: '#4A3B6B' });
    await check('set_color', 'done');
    await mockInvoke('set_size', { id: ID, size: 'wide' });
    await check('set_size', 'done');
    await mockInvoke('set_order', { id: ID, order: 1.5 });
    await check('set_order', 'done');
    await mockInvoke('snooze', { id: ID, minutes: 60 });
    await check('snooze', 'done');
    await mockInvoke('enrich_tip', { id: ID });
    await check('enrich_tip', 'done');
    let tip = await agentTip();
    await mockInvoke('update_tip', { id: ID, base: tip.sha ?? null, edit: formOf(tip) });
    await check('update_tip unchanged', 'done');
    tip = await agentTip();
    const changed = { ...formOf(tip), title: 'Renamed', priority: 'high' as const, tags: ['lit'], due: '2026-11-21 17:00', remind: ['-1d'], color: '#123456', size: 'sm', highlight: ['hairpin'] };
    await mockInvoke('update_tip', { id: ID, base: tip.sha ?? null, edit: changed });
    await check('update_tip all fields', 'done');
    tip = await agentTip();
    await mockInvoke('update_tip', { id: ID, base: tip.sha ?? null, edit: { ...formOf(tip), body: `${tip.body}\nAlso 2019, please.\n` } });
    await check('update_tip body', 'queued');
    tip = await agentTip();
    await mockInvoke('update_tip', { id: ID, base: tip.sha ?? null, edit: { ...formOf(tip), kind: 'task' } });
    await check('update_tip kind task', undefined);
    await mockInvoke('mark_done', { id: ID });
    await check('mark_done', undefined);
    await mockInvoke('reopen', { id: ID });
    await check('reopen', undefined);
  });

  it('new agent tips from the form or quick capture are new tasks', async () => {
    const full = await mockInvoke('create_tip_full', { edit: { ...emptyForm(), title: 'Agent task', kind: 'agent', body: 'Find three papers.' } });
    const made = full.tips.find((t) => t.title === 'Agent task');
    expect(made).toMatchObject({ kind: 'agent', body: 'Find three papers.', agent_state: 'queued' });
    expect(made?.agent).toBeUndefined();
    const blank = await mockInvoke('create_tip_full', { edit: { ...emptyForm(), title: 'Blank agent task', kind: 'agent' } });
    expect(blank.tips.find((t) => t.title === 'Blank agent task')?.body).toBe('');
    const captured = await mockInvoke('create_tip', { text: 'Captured agent task >agent' });
    const cap = captured.tips.find((t) => t.title === 'Captured agent task');
    expect(cap).toMatchObject({ kind: 'agent', agent_state: 'queued' });
    expect(cap?.agent).toBeUndefined();
  });
});

function emptyForm(): TipEdit {
  return { title: '', kind: 'task', priority: 'normal', due: null, location: null, tags: [], remind: [], repeat: null, links: [], images: [], color: null, size: null, highlight: [], body: '' };
}
