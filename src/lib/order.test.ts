import { describe, expect, it } from 'vitest';
import { applyManualOrder, orderForMove, sortKeys } from './order';

const ranked = [{ id: 'a' }, { id: 'b' }, { id: 'c' }, { id: 'd' }];

describe('sortKeys', () => {
  it('uses rank unless an order is set', () => {
    const keys = sortKeys([{ id: 'a' }, { id: 'b', order: 0.5 }, { id: 'c', order: Number.NaN }]);
    expect(keys.get('a')).toBe(0);
    expect(keys.get('b')).toBe(0.5);
    expect(keys.get('c')).toBe(2);
  });
});

describe('applyManualOrder', () => {
  it('keeps the backend order without overrides', () => {
    expect(applyManualOrder(ranked).map((t) => t.id)).toEqual(['a', 'b', 'c', 'd']);
  });
  it('interleaves explicit orders', () => {
    const list = [{ id: 'a' }, { id: 'b' }, { id: 'c', order: -1 }, { id: 'd', order: 0.5 }];
    expect(applyManualOrder(list).map((t) => t.id)).toEqual(['c', 'a', 'd', 'b']);
  });
});

describe('orderForMove', () => {
  it('moves to the top, bottom and between', () => {
    expect(orderForMove(ranked, 'c', 0)).toBe(-1);
    expect(orderForMove(ranked, 'a', 3)).toBe(4);
    expect(orderForMove(ranked, 'd', 1)).toBe(0.5);
  });
  it('returns undefined when nothing moves', () => {
    expect(orderForMove(ranked, 'b', 1)).toBeUndefined();
    expect(orderForMove(ranked, 'zzz', 0)).toBeUndefined();
  });
  it('clamps the target index', () => {
    expect(orderForMove(ranked, 'a', 99)).toBe(4);
    expect(orderForMove(ranked, 'd', -5)).toBe(-1);
  });
  it('composes: the moved tile lands where it was dropped', () => {
    const order = orderForMove(ranked, 'd', 1) as number;
    const after = ranked.map((t) => (t.id === 'd' ? { ...t, order } : t));
    expect(applyManualOrder(after).map((t) => t.id)).toEqual(['a', 'd', 'b', 'c']);
  });
});
