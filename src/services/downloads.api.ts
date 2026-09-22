import * as desktop from "../shared/api/desktop";
import { normalizeIpcError } from "../shared/api/desktop";
import type { DownloadTaskDto, PagedResult } from "../types/backend";
import type { QueueResultDto, DownloadTaskEvent } from "../types/downloads";
import type { UnlistenFn } from "@tauri-apps/api/event";

/// Phase 06 production boundary for the download engine — the single place
/// the UI reaches real queue state. Inputs are chapter/task ids only; page
/// URLs, headers and output paths are owned by Rust and never round-trip
/// through the renderer.
export interface DownloadManagerService {
  listTasks(args: { status?: string; limit: number; offset: number }): Promise<PagedResult<DownloadTaskDto>>;
  queue(args: { chapterIds: string[]; format?: "cbz" | "folder"; priority?: number }): Promise<QueueResultDto>;
  pause(taskId: string): Promise<DownloadTaskDto>;
  resume(taskId: string): Promise<DownloadTaskDto>;
  cancel(taskId: string): Promise<DownloadTaskDto>;
  retry(taskId: string): Promise<DownloadTaskDto>;
  setPriority(taskId: string, priority: number): Promise<DownloadTaskDto>;
  subscribe(handler: (event: DownloadTaskEvent) => void): Promise<UnlistenFn>;
}

function rethrow(err: unknown): never {
  throw new Error(normalizeIpcError(err).message);
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
  pause: (taskId) => desktop.pauseDownload(taskId).catch(rethrow),
  resume: (taskId) => desktop.resumeDownload(taskId).catch(rethrow),
  cancel: (taskId) => desktop.cancelDownload(taskId).catch(rethrow),
  retry: (taskId) => desktop.retryDownload(taskId).catch(rethrow),
  setPriority: (taskId, priority) => desktop.setDownloadPriority({ taskId, priority }).catch(rethrow),
  subscribe: (handler) => desktop.subscribeDownloadTask(handler),
};
