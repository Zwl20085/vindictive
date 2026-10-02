import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Tip } from '../types';
import { setLocale } from '../lib/i18n';
import { HOUR_MS, toNaive } from '../lib/time';
import { CELEBRATE_MS, Clawd, HOP_MS, isSleepTime } from './clawd';

afterEach(() => {
  vi.useRealTimers();
  setLocale('en');
});

const at = (hour: number): Date => new Date(2026, 9, 2, hour, 30);
const noon = at(12);

function tip(id: string, patch: Partial<Tip> = {}): Tip {
  return { id, path: `${id}.md`, title: id, kind: 'task', priority: 'normal', status: 'open', body: '', remind_at: [], ...patch };
}
const overdue = (id: string): Tip => tip(id, { due_at: toNaive(new Date(noon.getTime() - HOUR_MS)) });

function sprite(clawd: Clawd): HTMLElement {
  const node = clawd.element.querySelector<HTMLElement>('.clawd-sprite');
  if (!node) throw new Error('no sprite');
  return node;
}

describe('Clawd', () => {
  it('draws a pixel sprite with body, arms, eyes and two leg pairs', () => {
    const clawd = new Clawd();
    const svg = clawd.element.querySelector('svg.clawd');
    expect(svg?.getAttribute('shape-rendering')).toBe('crispEdges');
    expect(clawd.element.querySelectorAll('.clawd-body rect')).toHaveLength(1);
    expect(clawd.element.querySelectorAll('.clawd-arms-rest rect')).toHaveLength(2);
    expect(clawd.element.querySelectorAll('.clawd-eyes rect')).toHaveLength(2);
    expect(clawd.element.querySelectorAll('.clawd-legs')).toHaveLength(2);
    expect(clawd.element.querySelectorAll('.clawd-confetti').length).toBeGreaterThan(0);
  });

  it('hops when clicked and settles afterwards', () => {
    vi.useFakeTimers();
    const clawd = new Clawd();
    sprite(clawd).click();
    expect(clawd.element.classList.contains('clawd-hopping')).toBe(true);
    vi.advanceTimersByTime(HOP_MS + 1);
    expect(clawd.element.classList.contains('clawd-hopping')).toBe(false);
  });

  it('sleeps at night and wakes in the morning', () => {
    expect(isSleepTime(at(23))).toBe(true);
    expect(isSleepTime(at(3))).toBe(true);
    expect(isSleepTime(at(6))).toBe(false);
    expect(isSleepTime(at(14))).toBe(false);
    const clawd = new Clawd();
    clawd.update(at(2));
    expect(clawd.element.dataset.mood).toBe('sleeping');
    expect(sprite(clawd).title).toBe('Clawd · zzz');
    clawd.update(at(9));
    expect(clawd.element.dataset.mood).toBe('walking');
  });

  it('worries about overdue tips and relaxes when all is done', () => {
    const clawd = new Clawd();
    clawd.setTips([tip('a'), overdue('b'), overdue('c')], noon);
    expect(clawd.element.dataset.mood).toBe('worried');
    expect(clawd.element.dataset.level).toBe('1');
    expect(sprite(clawd).title).toBe('Clawd · 2 overdue');
    clawd.setTips([tip('a'), overdue('b'), overdue('c'), overdue('d')], noon);
    expect(clawd.element.dataset.level).toBe('2');
    // Snoozed tips do not count; a tip done elsewhere is no reason to party.
    clawd.setTips([tip('s', { snoozed_until_at: toNaive(new Date(noon.getTime() + HOUR_MS)) }), tip('x', { status: 'done' })], noon);
    expect(clawd.element.dataset.mood).toBe('relaxed');
    expect(sprite(clawd).title).toBe('Clawd · all done');
  });

  it('celebrates a completed tip for a moment, but not on the first board', () => {
    vi.useFakeTimers();
    vi.setSystemTime(noon);
    const clawd = new Clawd();
    clawd.setTips([tip('a', { status: 'done' }), tip('b')], noon);
    expect(clawd.element.dataset.mood).toBe('walking');
    clawd.setTips([tip('a', { status: 'done' }), tip('b', { status: 'done' })], noon);
    expect(clawd.element.dataset.mood).toBe('celebrating');
    // Clicking does not interrupt the party.
    sprite(clawd).click();
    expect(clawd.element.classList.contains('clawd-hopping')).toBe(false);
    vi.advanceTimersByTime(CELEBRATE_MS + 1);
    expect(clawd.element.dataset.mood).toBe('relaxed');
  });

  it('a celebration briefly wakes him at night', () => {
    vi.useFakeTimers();
    const night = at(23);
    vi.setSystemTime(night);
    const clawd = new Clawd();
    clawd.setTips([tip('a')], night);
    expect(clawd.element.dataset.mood).toBe('sleeping');
    clawd.setTips([tip('a', { status: 'done' })], night);
    expect(clawd.element.dataset.mood).toBe('celebrating');
    vi.advanceTimersByTime(CELEBRATE_MS + 1);
    expect(clawd.element.dataset.mood).toBe('sleeping');
  });

  it('only touches the DOM when the mood changes', async () => {
    const clawd = new Clawd();
    clawd.setTips([overdue('b')], noon);
    const records: MutationRecord[] = [];
    const observer = new MutationObserver((list) => records.push(...list));
    observer.observe(clawd.element, { attributes: true, subtree: true });
    for (let s = 0; s < 5; s++) clawd.update(new Date(noon.getTime() + s * 1000));
    await Promise.resolve();
    observer.disconnect();
    expect(records).toHaveLength(0);
  });

  it('can be held in one mood for screenshots', () => {
    const clawd = new Clawd();
    clawd.force('celebrating');
    clawd.setTips([overdue('b')], noon);
    expect(clawd.element.dataset.mood).toBe('celebrating');
    clawd.force(undefined);
    clawd.update(noon);
    expect(clawd.element.dataset.mood).toBe('worried');
  });

  it('speaks the UI language', () => {
    setLocale('zh');
    const clawd = new Clawd();
    clawd.setTips([], noon);
    expect(sprite(clawd).title).toBe('Clawd · 全部完成');
  });
});
