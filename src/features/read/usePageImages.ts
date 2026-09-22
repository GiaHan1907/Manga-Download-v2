import { useEffect, useMemo, useRef, useState } from "react";
import { prefetchWindow } from "./readerLogic";

export type PageFetch = (index: number) => Promise<{ bytes: Uint8Array; mime: string }>;

/// Bounded blob-URL cache keyed per chapter. Only pages in a window around the
/// current page are fetched; evicted object URLs are revoked so memory stays
/// flat through a 300-page chapter (no whole-archive load, ever).
/// In-flight bookkeeping lives in refs (not state): a fetch started by an
/// older window must still land in the cache, otherwise a fast page turn
/// could mark a page pending forever without anything re-triggering it.
export function usePageImages(
  chapterId: string,
  page: number,
  mode: "single" | "double" | "vertical",
  pageCount: number,
  fetchPage: PageFetch,
) {
  const [urls, setUrls] = useState<Record<number, string>>({});
  const [errors, setErrors] = useState<Record<number, string>>({});
  const cache = useRef(new Map<number, string>());
  const inflight = useRef(new Set<number>());
  const chapterRef = useRef(chapterId);
  const disposed = useRef(false);
  const [nonce, setNonce] = useState(0);

  // Reset when the chapter changes: object URLs from another archive are stale.
  useEffect(() => {
    chapterRef.current = chapterId;
    const stale = cache.current;
    cache.current = new Map();
    inflight.current = new Set();
    for (const url of stale.values()) {
      URL.revokeObjectURL(url);
    }
    // Deferred out of the effect body (no synchronous cascade).
    void Promise.resolve().then(() => {
      setUrls({});
      setErrors({});
    });
  }, [chapterId]);

  const radius = mode === "vertical" ? 3 : 1;
  const wanted = useMemo(
    () => prefetchWindow(page, pageCount, radius),
    [page, pageCount, radius],
  );

  useEffect(() => {
    for (const index of wanted) {
      if (cache.current.has(index) || inflight.current.has(index) || errors[index] !== undefined) {
        continue;
      }
      inflight.current.add(index);
      const forChapter = chapterId;
      void fetchPage(index)
        .then(({ bytes, mime }) => {
          inflight.current.delete(index);
          if (chapterRef.current !== forChapter) {
            return;
          }
          const blob = new Blob([bytes as unknown as BlobPart], { type: mime });
          const url = URL.createObjectURL(blob);
          // A fetch resolving after unmount must not seed the revoked cache —
          // revoke immediately instead of leaking the object URL.
          if (disposed.current) {
            URL.revokeObjectURL(url);
            return;
          }
          cache.current.set(index, url);
          setUrls((prev) => ({ ...prev, [index]: url }));
        })
        .catch((error: unknown) => {
          inflight.current.delete(index);
          if (disposed.current || chapterRef.current !== forChapter) {
            return;
          }
          setErrors((prev) => ({
            ...prev,
            [index]: error instanceof Error ? error.message : "Không tải được trang.",
          }));
        });
    }
    // Evict pages far outside the window (bounded retention).
    const keep = new Set(wanted);
    for (const [index, url] of [...cache.current.entries()]) {
      if (!keep.has(index)) {
        URL.revokeObjectURL(url);
        cache.current.delete(index);
        setUrls((prev) => {
          const next = { ...prev };
          delete next[index];
          return next;
        });
      }
    }
    // fetchPage is stable per chapter; the window string drives re-runs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted.join(","), chapterId, nonce]);

  // Revoke everything on unmount.
  useEffect(() => {
    disposed.current = false;
    return () => {
      disposed.current = true;
      const live = cache.current;
      for (const url of live.values()) {
        URL.revokeObjectURL(url);
      }
      live.clear();
    };
  }, []);

  function retry(index: number) {
    setErrors((prev) => {
      const next = { ...prev };
      delete next[index];
      return next;
    });
    setNonce((value) => value + 1);
  }

  return { urls, errors, retry };
}
