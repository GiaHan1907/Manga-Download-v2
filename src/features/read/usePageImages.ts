import { useEffect, useMemo, useRef, useState } from "react";
import { prefetchWindow } from "./readerLogic";

export type PageFetch = (index: number) => Promise<{ bytes: Uint8Array; mime: string }>;

/// Bounded blob-URL cache keyed per chapter. Only pages in a window around the
/// current page are fetched; evicted object URLs are revoked so memory stays
/// flat through a 300-page chapter (no whole-archive load, ever).
export function usePageImages(
  chapterId: string,
  page: number,
  mode: "single" | "double" | "vertical",
  pageCount: number,
  fetchPage: PageFetch,
) {
  const [urls, setUrls] = useState<Record<number, string>>({});
  const [errors, setErrors] = useState<Record<number, string>>({});
  const [pending, setPending] = useState<Record<number, boolean>>({});
  const cache = useRef(new Map<number, string>());
  const [nonce, setNonce] = useState(0);

  // Reset when the chapter changes: object URLs from another archive are stale.
  useEffect(() => {
    const stale = cache.current;
    cache.current = new Map();
    for (const url of stale.values()) {
      URL.revokeObjectURL(url);
    }
    // Deferred out of the effect body (no synchronous cascade).
    void Promise.resolve().then(() => {
      setUrls({});
      setErrors({});
      setPending({});
    });
  }, [chapterId]);

  const radius = mode === "vertical" ? 3 : 1;
  const wanted = useMemo(
    () => prefetchWindow(page, pageCount, radius),
    [page, pageCount, radius],
  );

  useEffect(() => {
    let cancelled = false;
    for (const index of wanted) {
      if (cache.current.has(index) || pending[index] || errors[index] !== undefined) {
        continue;
      }
      setPending((prev) => ({ ...prev, [index]: true }));
      void fetchPage(index)
        .then(({ bytes, mime }) => {
          if (cancelled) {
            return;
          }
          const blob = new Blob([bytes as unknown as BlobPart], { type: mime });
          const url = URL.createObjectURL(blob);
          cache.current.set(index, url);
          setUrls((prev) => ({ ...prev, [index]: url }));
          setPending((prev) => ({ ...prev, [index]: false }));
        })
        .catch((error: unknown) => {
          if (cancelled) {
            return;
          }
          setErrors((prev) => ({
            ...prev,
            [index]: error instanceof Error ? error.message : "Không tải được trang.",
          }));
          setPending((prev) => ({ ...prev, [index]: false }));
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
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted.join(","), chapterId, nonce]);

  // Revoke everything on unmount.
  useEffect(() => {
    const local = cache.current;
    return () => {
      for (const url of local.values()) {
        URL.revokeObjectURL(url);
      }
      local.clear();
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

  return { urls, errors, pending, retry };
}
