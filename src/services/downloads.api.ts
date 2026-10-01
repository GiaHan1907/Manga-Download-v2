import * as desktop from "../shared/api/desktop";
import { normalizeIpcError } from "../shared/api/desktop";
import type { DownloadTaskDto, PagedResult } from "../types/backend";
import type { QueueResultDto, DownloadTaskEvent, ClearHistoryDto } from "../types/downloads";
import type { UnlistenFn } from "@tauri-apps/api/event";

/// Phase 06 production boundary for the download engine — the single place
/// the UI reaches real queue state. Inputs are chapter/task ids only; page
/// URLs, headers and output paths are owned by Rust and never round-trip
/// through the renderer.
export interface DownloadManagerService {
  listTasks(args: { status?: string; limit: number; offset: number }): Promise<PagedResult<DownloadTaskDto>>;
  queue(args: { chapterIds: string[]; format?: "cbz" | "folder"; priority?: number }): Promise<QueueResultDto>;
  /// Queue an arbitrary number of chapters: the Rust gate allows 100 ids per
  /// call, so this chunks sequentially and aggregates one honest result.
  queueMany(args: { chapterIds: string[]; format?: "cbz" | "folder" }): Promise<QueueResultDto>;
  pause(taskId: string): Promise<DownloadTaskDto>;
  resume(taskId: string): Promise<DownloadTaskDto>;
  cancel(taskId: string): Promise<DownloadTaskDto>;
  retry(taskId: string): Promise<DownloadTaskDto>;
  /// Drop one settled task row from the list (no file is ever deleted).
  removeTask(taskId: string): Promise<boolean>;
  setPriority(taskId: string, priority: number): Promise<DownloadTaskDto>;
  clearHistory(args: { statuses?: string[]; olderThanDays?: number }): Promise<ClearHistoryDto>;
  retryAllFailed(): Promise<number>;
  cancelForManga(mangaId: string): Promise<number>;
  setQueuePaused(paused: boolean): Promise<number>;
  getQueuePaused(): Promise<boolean>;
  subscribe(handler: (event: DownloadTaskEvent) => void): Promise<UnlistenFn>;
}

function rethrow(err: unknown): never {
  throw new Error(normalizeIpcError(err).message);
}

/// Mirror of Rust `download_repo::MAX_BATCH_SIZE` — keep in sync.
const QUEUE_BATCH_LIMIT = 100;

async function queueMany(args: {
  chapterIds: string[];
  format?: "cbz" | "folder";
}): Promise<QueueResultDto> {
  const aggregated: QueueResultDto = {
    tasks: [],
    reused: [],
    errors: [],
    queuedAt: new Date().toISOString(),
  };
  for (let offset = 0; offset < args.chapterIds.length; offset += QUEUE_BATCH_LIMIT) {
    const chunk = args.chapterIds.slice(offset, offset + QUEUE_BATCH_LIMIT);
    const outcome = await downloadsApi.queue({ chapterIds: chunk, format: args.format });
    aggregated.tasks.push(...outcome.tasks);
    aggregated.reused.push(...outcome.reused);
    aggregated.errors.push(...outcome.errors);
    aggregated.queuedAt = outcome.queuedAt;
  }
  return aggregated;
}

export const downloadsApi: DownloadManagerService = {
  listTasks: (args) => desktop.listLocalDownloadTasks(args).catch(rethrow),
  queue: (args) =>
    desktop
      .queueChapterDownloads({
        chapterIds: args.chapterIds,
        format: args.format,
        priority: args.priority,
      })
      .catch(rethrow),
  queueMany: (args) => queueMany(args).catch(rethrow),
  pause: (taskId) => desktop.pauseDownload(taskId).catch(rethrow),
  resume: (taskId) => desktop.resumeDownload(taskId).catch(rethrow),
  cancel: (taskId) => desktop.cancelDownload(taskId).catch(rethrow),
  retry: (taskId) => desktop.retryDownload(taskId).catch(rethrow),
  removeTask: (taskId) => desktop.removeDownloadTask(taskId).catch(rethrow),
  setPriority: (taskId, priority) => desktop.setDownloadPriority({ taskId, priority }).catch(rethrow),
  clearHistory: (args) => desktop.clearDownloadHistory(args).catch(rethrow),
  retryAllFailed: () => desktop.retryAllFailedDownloads().catch(rethrow),
  cancelForManga: (mangaId) => desktop.cancelDownloadsForManga(mangaId).catch(rethrow),
  setQueuePaused: (paused) => desktop.setDownloadQueuePaused(paused).catch(rethrow),
  getQueuePaused: () => desktop.getDownloadQueuePaused().catch(rethrow),
  subscribe: (handler) => desktop.subscribeDownloadTask(handler),
};
