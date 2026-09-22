import { useEffect, useRef } from "react";
import type { ReaderManifestDto } from "../../types/library";
import { localServices } from "../../services/provider";

const DEBOUNCE_MS = 800;

/// Progress persistence policy (design §P07-07): page movement writes are
/// DEBOUNCED (one write per pause in navigation, never one per key press);
/// chapter switches and reader close flush the latest position immediately.
/// Failures never fake success — the next legitimate change retries.
export function useReaderProgress(
  manifest: { state: string; data?: ReaderManifestDto },
  page: number,
) {
  const timer = useRef<number | null>(null);
  const latest = useRef<{ manifest: ReaderManifestDto; page: number } | null>(null);

  // Keep the flush snapshot current after every committed render (refs must
  // not be written during render itself).
  useEffect(() => {
    if (manifest.state === "success" && manifest.data) {
      latest.current = { manifest: manifest.data, page };
    }
  });

  const chapterId = manifest.state === "success" ? manifest.data?.chapterId ?? null : null;

  // Debounced write while paging.
  useEffect(() => {
    if (manifest.state !== "success" || !manifest.data) {
      return;
    }
    const { mangaId, chapterId: cid, pageCount } = manifest.data;
    timer.current = window.setTimeout(() => {
      void localServices.reader
        .saveProgress({ mangaId, chapterId: cid, pageIndex: page, pageTotal: pageCount })
        .catch(() => undefined);
    }, DEBOUNCE_MS);
    return () => {
      if (timer.current) {
        window.clearTimeout(timer.current);
      }
    };
  }, [manifest, page]);

  // Immediate flush on chapter change or unmount (reader close).
  useEffect(
    () => () => {
      const snapshot = latest.current;
      if (!snapshot) {
        return;
      }
      void localServices.reader
        .saveProgress({
          mangaId: snapshot.manifest.mangaId,
          chapterId: snapshot.manifest.chapterId,
          pageIndex: snapshot.page,
          pageTotal: snapshot.manifest.pageCount,
        })
        .catch(() => undefined);
    },
    [chapterId],
  );
}
