import type { Tip } from '../types';
import { parseNaive } from './time';
import { urgencyOf } from './urgency';

export type TileSize = 'sm' | 'md' | 'wide';

/** Grid units per size, `[columns, rows]`. */
export const TILE_SPANS: Record<TileSize, readonly [number, number]> = {
  sm: [1, 1],
  md: [2, 2],
  wide: [4, 2],
};

/**
 * `wide` for the next-up tip, `md` for high priority or a deadline within
 * seven days, `sm` otherwise.
 */
export function tileSize(tip: Tip, isNextUp: boolean, now: Date): TileSize {
  if (isNextUp) return 'wide';
  if (tip.priority === 'high') return 'md';
  const urgency = urgencyOf(parseNaive(tip.due_at), now);
  const dueSoon = urgency === 'soon' || urgency === 'critical' || urgency === 'overdue';
  if (tip.kind === 'deadline' && dueSoon) return 'md';
  return 'sm';
}

/** Clamp a span so it never exceeds the available columns. */
export function spanFor(size: TileSize, columns: number): readonly [number, number] {
  const [cols, rows] = TILE_SPANS[size];
  return [Math.max(1, Math.min(cols, columns)), rows];
}
