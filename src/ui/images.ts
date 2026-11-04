/**
 * Frontend cache in front of `fetch_image`. The backend already caches data
 * URLs, but tiles are re-rendered every 30 s and should not round-trip IPC
 * for the same thumbnail each time.
 *
 * Entries are keyed by source path *and* the owning tip's `sha`, so editing a
 * tip (which changes its sha) fetches its figures again. Entries for shas no
 * longer on the board are dropped by `pruneImageCache`, and the cache never
 * holds more than `IMAGE_CACHE_MAX` entries (oldest use evicted first).
 */
import type { ImageResolver } from '../lib/markdown';

/** Resolve `src`; `version` (the tip's sha) is part of the cache key. */
export type VersionedResolver = (src: string, version?: string) => Promise<string>;

export const IMAGE_CACHE_MAX = 200;

const SEP = '\u0000';
const cache = new Map<string, Promise<string>>();

function keyOf(src: string, version: string | undefined): string {
  return `${version ?? ''}${SEP}${src}`;
}

export function cachedResolver(resolve: ImageResolver): VersionedResolver {
  return (src: string, version?: string) => {
    const path = src.trim();
    const key = keyOf(path, version);
    let pending = cache.get(key);
    if (pending) {
      // Refresh recency: Map iteration order is insertion order.
      cache.delete(key);
      cache.set(key, pending);
      return pending;
    }
    pending = resolve(path).catch((error) => {
      if (cache.get(key) === pending) cache.delete(key);
      throw error;
    });
    cache.set(key, pending);
    while (cache.size > IMAGE_CACHE_MAX) {
      const oldest = cache.keys().next().value;
      if (oldest === undefined) break;
      cache.delete(oldest);
    }
    return pending;
  };
}

/**
 * Drop entries cached for a tip version that is no longer on the board.
 * Unversioned entries are left to the size cap.
 */
export function pruneImageCache(liveVersions: ReadonlySet<string>): void {
  for (const key of Array.from(cache.keys())) {
    const version = key.slice(0, key.indexOf(SEP));
    if (version && !liveVersions.has(version)) cache.delete(key);
  }
}

export function imageCacheSize(): number {
  return cache.size;
}

export function clearImageCache(): void {
  cache.clear();
}
