/**
 * Manual tile order. The backend ranks tips by score; a tip may carry an
 * explicit `order` in its frontmatter. Sort keys are `order` when present,
 * otherwise the tip's rank in the backend list, so an explicit value such as
 * 2.5 lands between the third and fourth ranked tips. Dropping a tile
 * between two neighbours gives it the midpoint of their keys: one file
 * changes, one commit.
 */

export interface Orderable {
  id: string;
  order?: number;
}

/** Sort key per id: explicit `order`, else the rank in `ranked`. */
export function sortKeys(ranked: readonly Orderable[]): Map<string, number> {
  const keys = new Map<string, number>();
  ranked.forEach((tip, index) => keys.set(tip.id, typeof tip.order === 'number' && Number.isFinite(tip.order) ? tip.order : index));
  return keys;
}

/** `ranked` reordered by sort key; ties keep the backend order. */
export function applyManualOrder<T extends Orderable>(ranked: readonly T[]): T[] {
  const keys = sortKeys(ranked);
  return ranked
    .map((tip, index) => ({ tip, index }))
    .sort((a, b) => (keys.get(a.tip.id) ?? a.index) - (keys.get(b.tip.id) ?? b.index) || a.index - b.index)
    .map((x) => x.tip);
}

/**
 * The `order` that moves `id` to position `toIndex` of the displayed list
 * (index among the *other* tiles, 0 = first). Returns `undefined` when the
 * tile is already there.
 */
export function orderForMove(ranked: readonly Orderable[], id: string, toIndex: number): number | undefined {
  const keys = sortKeys(ranked);
  const shown = applyManualOrder(ranked);
  const from = shown.findIndex((t) => t.id === id);
  if (from < 0) return undefined;
  const rest = shown.filter((t) => t.id !== id);
  const at = Math.max(0, Math.min(toIndex, rest.length));
  if (at === from) return undefined;
  const prev = rest[at - 1];
  const next = rest[at];
  const prevKey = prev ? keys.get(prev.id) ?? 0 : undefined;
  const nextKey = next ? keys.get(next.id) ?? 0 : undefined;
  if (prevKey === undefined && nextKey === undefined) return undefined;
  if (prevKey === undefined) return (nextKey as number) - 1;
  if (nextKey === undefined) return prevKey + 1;
  return (prevKey + nextKey) / 2;
}
