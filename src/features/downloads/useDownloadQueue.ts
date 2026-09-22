import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DownloadTaskDto } from "../../types/backend";
import type { DownloadTaskEvent } from "../../types/downloads";
import { downloadsApi } from "../../services/downloads.api";

/// Client-side mirror of the Rust queue: an authoritative snapshot per page,
/// then merged event frames guarded by per-task `revision` so an out-of-order
/// event can never roll back newer state. Actions refetch after the backend
/// acknowledges (no optimistic completion — design §8.2). History beyond the
/// page window is reachable via `offset`/`total` (bounded paging, Phase 08).
export const QUEUE_PAGE_SIZE = 100;

export function useDownloadQueue() {
  const [tasks, setTasks] = useState<DownloadTaskDto[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [live, setLive] = useState<Record<string, { bytes: number; speed: number | null }>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadNonce, setReloadNonce] = useState(0);
  const revisionRef = useRef<Map<string, number>>(new Map());
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const page = await downloadsApi.listTasks({ limit: QUEUE_PAGE_SIZE, offset });
      if (!mounted.current) return;
      for (const task of page.items) {
        revisionRef.current.set(task.id, task.revision);
      }
      setTasks(page.items);
      setTotal(page.total);
    } catch (err) {
      if (mounted.current) setError(err instanceof Error ? err.message : "Không tải được hàng đợi.");
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, [offset]);

  useEffect(() => {
    // Deferred so the first setState happens outside the synchronous effect
    // body (avoids render cascades; same pattern the async hooks rely on).
    void Promise.resolve().then(refresh);
  }, [refresh, reloadNonce]);

  // Subscribe once; re-query on reconnect focuses handled by actions/refresh.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void downloadsApi
      .subscribe((event: DownloadTaskEvent) => {
        if (cancelled || !mounted.current) return;
        const seen = revisionRef.current.get(event.task.id) ?? -1;
        if (event.task.revision < seen) {
          return; // stale frame — authoritative snapshot wins
        }
        revisionRef.current.set(event.task.id, event.task.revision);
        setLive((prev) => ({
          ...prev,
          [event.task.id]: { bytes: event.downloadedBytes, speed: event.speedBytesPerSecond },
        }));
        setTasks((prev) => {
          const next = prev.map((task) => (task.id === event.task.id ? event.task : task));
          if (!prev.some((task) => task.id === event.task.id)) {
            next.unshift(event.task);
          }
          return next;
        });
      })
      .then((dispose) => {
        if (cancelled) dispose();
        else unlisten = dispose;
      })
      .catch(() => {
        /* listener wiring only fails outside Tauri (browser demo) */
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const applyAction = useCallback(
    async (taskId: string, action: (id: string) => Promise<DownloadTaskDto>) => {
      try {
        const updated = await action(taskId);
        revisionRef.current.set(updated.id, Math.max(revisionRef.current.get(updated.id) ?? 0, updated.revision));
        setTasks((prev) => prev.map((task) => (task.id === updated.id ? updated : task)));
        // Re-fetch authoritative snapshot after each action.
        await refresh();
        return updated;
      } catch (err) {
        setError(err instanceof Error ? err.message : "Thao tác bị từ chối.");
        throw err;
      }
    },
    [refresh],
  );

  const actions = useMemo(
    () => ({
      pause: (id: string) => applyAction(id, downloadsApi.pause),
      resume: (id: string) => applyAction(id, downloadsApi.resume),
      cancel: (id: string) => applyAction(id, downloadsApi.cancel),
      retry: (id: string) => applyAction(id, downloadsApi.retry),
      setPriority: (id: string, priority: number) =>
        applyAction(id, (taskId) => downloadsApi.setPriority(taskId, priority)),
    }),
    [applyAction],
  );

  return {
    tasks,
    live,
    loading,
    error,
    actions,
    total,
    offset,
    goPage: (nextOffset: number) => setOffset(Math.max(0, nextOffset)),
    refresh: () => setReloadNonce((n) => n + 1),
  };
}
