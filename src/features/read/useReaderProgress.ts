import { useEffect, useRef } from "react";
import type { RefObject } from "react";
import type { ReaderManifestDto } from "../../types/library";
import { localServices } from "../../services/provider";
import { bumpDomain } from "../../shared/hooks/useInvalidation";

const DEBOUNCE_MS = 800;

/// Progress persistence policy (design §P07-07): page movement writes are
/// DEBOUNCED (one write per pause in navigation, never one per key press);
/// chapter switches and reader close flush the latest position immediately.
/// Failures never fake success — the next legitimate change retries.
export function useReaderProgress(
  manifest: { state: string; data?: ReaderManifestDto },
  page: number,
  scrollInfo: RefObject<{ page: number; fraction: number } | null>,
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
      // The scroll fraction is only valid while it still describes the page
      // being written; a stale measurement must never leak into progress.
      const measured = scrollInfo.current;
      const scrollOffset = measured && measured.page === page ? measured.fraction : undefined;
      void localServices.reader
        .saveProgress({
          mangaId,
          chapterId: cid,
          pageIndex: page,
          pageTotal: pageCount,
          scrollOffset,
        })
        .then(() => {
          // Cross-screen: Home/Library Continue Reading reflects the new spot.
          bumpDomain("home");
          bumpDomain("library");
        })
        .catch(() => undefined);
    }, DEBOUNCE_MS);
    return () => {
      if (timer.current) {
        window.clearTimeout(timer.current);
      }
    };
  }, [manifest, page, scrollInfo]);

  // Immediate flush on chapter change or unmount (reader close).
  useEffect(
    () => () => {
      const snapshot = latest.current;
      if (!snapshot) {
        return;
      }
      const measured = scrollInfo.current;
      const scrollOffset =
        measured && measured.page === snapshot.page ? measured.fraction : undefined;
      void localServices.reader
        .saveProgress({
          mangaId: snapshot.manifest.mangaId,
          chapterId: snapshot.manifest.chapterId,
          pageIndex: snapshot.page,
          pageTotal: snapshot.manifest.pageCount,
          scrollOffset,
        })
        .catch(() => undefined);
    },
    [chapterId, scrollInfo],
  );
}
