import { useEffect, useState } from "react";
import * as desktop from "../api/desktop";
import { sniffImageMime } from "../lib/imageMime";

const CACHE_LIMIT = 120;
// Object URLs are cached module-wide, keyed by source+manga+cover so grid
// re-renders and scroll do not refetch. Eviction revokes the oldest blob.
const cache = new Map<string, string>();
const inflight = new Map<string, Promise<string | null>>();

function remember(key: string, url: string) {
  if (cache.has(key)) {
    cache.delete(key);
  } else if (cache.size >= CACHE_LIMIT) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) {
      URL.revokeObjectURL(cache.get(oldest) as string);
      cache.delete(oldest);
    }
  }
  cache.set(key, url);
}

function load(cacheKey: string, sourceId: string, sourceMangaId: string, coverKey: string) {
  const pending = inflight.get(cacheKey);
  if (pending) {
    return pending;
  }
  const request = desktop
    .getMangaCover({ sourceId, sourceMangaId, coverKey })
    .then((bytes) => {
      const url = URL.createObjectURL(
        new Blob([bytes as unknown as BlobPart], { type: sniffImageMime(bytes) }),
      );
      remember(cacheKey, url);
      return url;
    })
    .catch(() => null)
    .finally(() => inflight.delete(cacheKey));
  inflight.set(cacheKey, request);
  return request;
}

/// Resolves a remote cover to an object-URL, or null when there is no cover
/// key / the fetch is pending / it failed. Cache hits are derived directly at
/// render time; only async completions touch state. Failures are not cached,
/// so a later render retries.
export function useCoverUrl(
  sourceId: string,
  sourceMangaId: string,
  coverKey: string | null,
): string | null {
  const cacheKey = coverKey ? `${sourceId}:${sourceMangaId}:${coverKey}` : null;
  const [resolved, setResolved] = useState<{ key: string; url: string } | null>(null);

  useEffect(() => {
    if (!cacheKey || !coverKey) {
      return;
    }
    if (cache.has(cacheKey)) {
      return;
    }
    let active = true;
    void load(cacheKey, sourceId, sourceMangaId, coverKey).then((url) => {
      if (active && url) {
        setResolved({ key: cacheKey, url });
      }
    });
    return () => {
      active = false;
    };
  }, [cacheKey, coverKey, sourceId, sourceMangaId]);

  if (!cacheKey) {
    return null;
  }
  if (resolved && resolved.key === cacheKey) {
    return resolved.url;
  }
  return cache.get(cacheKey) ?? null;
}
