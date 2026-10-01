import { afterEach, describe, expect, it, vi } from 'vitest';
import { Clawd, HOP_MS, isSleepTime } from './clawd';

afterEach(() => vi.useRealTimers());

const at = (hour: number): Date => new Date(2026, 9, 2, hour, 30);

describe('Clawd', () => {
  it('draws a pixel sprite with body, eyes and two leg pairs', () => {
    const clawd = new Clawd();
    const svg = clawd.element.querySelector('svg.clawd');
    expect(svg?.getAttribute('shape-rendering')).toBe('crispEdges');
    expect(clawd.element.querySelectorAll('.clawd-body rect')).toHaveLength(3);
    expect(clawd.element.querySelectorAll('.clawd-eyes rect')).toHaveLength(2);
    expect(clawd.element.querySelectorAll('.clawd-legs')).toHaveLength(2);
  });

  it('hops when clicked and settles afterwards', () => {
    vi.useFakeTimers();
    const clawd = new Clawd();
    clawd.element.querySelector<HTMLElement>('.clawd-sprite')?.click();
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
    expect(clawd.element.dataset.sleeping).toBe('true');
    clawd.update(at(9));
    expect(clawd.element.dataset.sleeping).toBe('false');
  });
});
