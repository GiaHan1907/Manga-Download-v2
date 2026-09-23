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
  // Ring buffer (≤30 samples/task) of measured speeds for sparklines;
  // terminal tasks drop their samples (bounded memory, page lifetime only).
  const [speedHistory, setSpeedHistory] = useState<Record<string, number[]>>({});
  const [etaSeconds, setEtaSeconds] = useState<Record<string, number | null>>({});
  const [actionErrors, setActionErrors] = useState<Record<string, string>>({});
  const progressSample = useRef<Map<string, { pages: number; ts: number }>>(new Map());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadNonce, setReloadNonce] = useState(0);
  const revisionRef = useRef<Map<string, number>>(new Map());
  const latestRef = useRef<Map<string, DownloadTaskDto>>(new Map());
  const requestSeq = useRef(0);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  /// Monotonic merge: the highest-revision DTO seen per task wins, whether it
  /// arrived as an event frame or inside an (older) snapshot.
  function note(task: DownloadTaskDto): DownloadTaskDto {
    const seen = revisionRef.current.get(task.id) ?? -1;
    if (task.revision < seen) {
      return latestRef.current.get(task.id) ?? task;
    }
    revisionRef.current.set(task.id, task.revision);
    latestRef.current.set(task.id, task);
    return task;
  }

  const refresh = useCallback(async () => {
    const seq = ++requestSeq.current;
    setLoading(true);
    setError(null);
    try {
      const page = await downloadsApi.listTasks({ limit: QUEUE_PAGE_SIZE, offset });
      // Stale-response guard: only the newest request may commit.
      if (!mounted.current || seq !== requestSeq.current) return;
      setTasks(page.items.map(note));
      setTotal(page.total);
    } catch (err) {
      if (mounted.current && seq === requestSeq.current) {
        setError(err instanceof Error ? err.message : "Không tải được hàng đợi.");
      }
    } finally {
      if (mounted.current && seq === requestSeq.current) setLoading(false);
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
        const merged = note(event.task);
        if (merged !== event.task) {
          return; // stale frame — authoritative snapshot wins
        }
        const { id, completedPages, totalPages, status } = event.task;
        setLive((prev) => ({
          ...prev,
          [id]: { bytes: event.downloadedBytes, speed: event.speedBytesPerSecond },
        }));
        // A newer revision means the previous action rejection is obsolete.
        setActionErrors((prev) => {
          if (prev[id] === undefined) return prev;
          const next = { ...prev };
          delete next[id];
          return next;
        });
        const terminal = status === "completed" || status === "failed" || status === "cancelled";
        if (terminal) {
          progressSample.current.delete(id);
          setSpeedHistory((prev) => {
            if (prev[id] === undefined) return prev;
            const next = { ...prev };
            delete next[id];
            return next;
          });
          setEtaSeconds((prev) => {
            if (prev[id] === undefined) return prev;
            const next = { ...prev };
            delete next[id];
            return next;
          });
        } else {
          if (event.speedBytesPerSecond !== null) {
            const sample = event.speedBytesPerSecond;
            setSpeedHistory((prev) => ({
              ...prev,
              [id]: [...(prev[id] ?? []), sample].slice(-30),
            }));
          }
          // ETA from page progression (rate of completedPages over wall time).
          const now = Date.now();
          const last = progressSample.current.get(id);
          progressSample.current.set(id, { pages: completedPages, ts: now });
          if (last && completedPages > last.pages && totalPages > completedPages) {
            const elapsedSeconds = (now - last.ts) / 1000;
            const pagesPerSecond = (completedPages - last.pages) / elapsedSeconds;
            if (pagesPerSecond > 0) {
              const remaining = totalPages - completedPages;
              setEtaSeconds((prev) => ({ ...prev, [id]: Math.ceil(remaining / pagesPerSecond) }));
            }
          }
        }
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
        const merged = note(updated);
        setTasks((prev) => prev.map((task) => (task.id === updated.id ? merged : task)));
        // Re-fetch authoritative snapshot after each action.
        await refresh();
        return updated;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Thao tác bị từ chối.";
        // Inline per-card feedback (self-clears on the task's next event).
        setActionErrors((prev) => ({ ...prev, [taskId]: message }));
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
    speedHistory,
    etaSeconds,
    actionErrors,
    loading,
    error,
    actions,
    total,
    offset,
    goPage: (nextOffset: number) => setOffset(Math.max(0, nextOffset)),
    refresh: () => setReloadNonce((n) => n + 1),
  };
}
