/**
 * Frontend cache in front of `fetch_image`. The backend already caches data
 * URLs, but tiles are re-rendered every 30 s and should not round-trip IPC
 * for the same thumbnail each time.
 */
import type { ImageResolver } from '../lib/markdown';

const cache = new Map<string, Promise<string>>();

export function cachedResolver(resolve: ImageResolver): ImageResolver {
  return (src: string) => {
    const key = src.trim();
    let pending = cache.get(key);
    if (!pending) {
      pending = resolve(key).catch((error) => {
        cache.delete(key);
        throw error;
      });
      cache.set(key, pending);
    }
    return pending;
  };
}

export function clearImageCache(): void {
  cache.clear();
}
