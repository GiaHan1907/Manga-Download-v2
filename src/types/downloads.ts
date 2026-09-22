import type { DownloadTaskDto } from "./backend";

/// Phase 06 — download engine contracts mirrored from the Rust Serde DTOs.
/// The renderer only ever sends chapter ids and lifecycle actions; page URLs,
/// headers and filesystem paths stay inside Rust.

export type DownloadOutputFormat = "cbz" | "folder";

export type QueueErrorDto = {
  chapterId: string;
  code: string;
  message: string;
};

export type QueueResultDto = {
  tasks: DownloadTaskDto[];
  reused: DownloadTaskDto[];
  errors: QueueErrorDto[];
  queuedAt: string;
};

export const DOWNLOAD_EVENT = "download-task-updated";

export type DownloadTaskEvent = {
  task: DownloadTaskDto;
  downloadedBytes: number;
  speedBytesPerSecond: number | null;
  occurredAt: string;
};

export const ACTIVE_STATUSES: ReadonlySet<DownloadTaskDto["status"]> = new Set([
  "resolving",
  "downloading",
  "pausing",
  "verifying",
  "exporting",
]);

export function isActiveStatus(status: DownloadTaskDto["status"]): boolean {
  return ACTIVE_STATUSES.has(status) || status === "queued";
}

/// Transition table mirroring the Rust lifecycle: UI actions are advisory —
/// the backend remains the authority (buttons merely hide illegal actions).
export function allowedDownloadActions(status: DownloadTaskDto["status"]): ("pause" | "resume" | "cancel" | "retry")[] {
  switch (status) {
    case "queued":
      return ["pause", "cancel"];
    case "resolving":
    case "downloading":
      return ["pause", "cancel"];
    case "verifying":
    case "exporting":
      // Backend commit window: pause is rejected there (scheduler.rs),
      // cancel is honored until the artifact commit.
      return ["cancel"];
    case "pausing":
      // Worker acknowledgement pending — resume would race the pause commit;
      // the backend only accepts resume from `paused`.
      return ["cancel"];
    case "paused":
      return ["resume", "cancel"];
    case "failed":
      return ["retry", "cancel"];
    default:
      return [];
  }
}

export function progressPercent(task: DownloadTaskDto): number {
  if (task.totalPages <= 0) {
    return 0;
  }
  return Math.min(100, Math.round((task.completedPages / task.totalPages) * 100));
}

export function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ["KB", "MB", "GB"];
  let size = value / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(1)} ${units[unit]}`;
}

export function formatSpeed(bytesPerSecond: number | null): string | null {
  if (bytesPerSecond === null || bytesPerSecond <= 0) {
    return null;
  }
  return `${formatBytes(bytesPerSecond)}/s`;
}
