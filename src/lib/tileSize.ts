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

export const TILE_SIZES: readonly TileSize[] = ['sm', 'md', 'wide'];

export function isTileSize(value: unknown): value is TileSize {
  return typeof value === 'string' && (TILE_SIZES as readonly string[]).includes(value);
}

/**
 * An explicit `size` wins. Otherwise `wide` for the next-up tip, `md` for
 * high priority or a deadline within seven days, `sm` for the rest.
 */
export function tileSize(tip: Tip, isNextUp: boolean, now: Date): TileSize {
  if (isTileSize(tip.size)) return tip.size;
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
