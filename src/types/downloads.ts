import type { DownloadTaskDto } from "./backend";

/// Phase 06 — download engine contracts mirrored from the Rust Serde DTOs.
/// The renderer only ever sends chapter ids and lifecycle actions; page URLs,
/// headers and filesystem paths stay inside Rust.

export type DownloadOutputFormat = "cbz" | "folder";

export const DOWNLOAD_STATUS_LABEL: Record<DownloadTaskDto["status"], string> = {
  queued: "Trong hàng đợi",
  resolving: "Đang phân giải",
  downloading: "Đang tải",
  pausing: "Đang dừng",
  paused: "Tạm dừng",
  verifying: "Đang kiểm tra",
  exporting: "Đang đóng gói",
  completed: "Hoàn thành",
  failed: "Thất bại",
  cancelled: "Đã hủy",
};

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

/// Feature plan F2 — purge result mirror (camelCase via serde).
/// `skippedArtifact`: completed rows whose artifact is still on disk (kept on
/// purpose); `skippedOther`: rows a concurrent action moved out of scope.
export type ClearHistoryDto = {
  deleted: number;
  skippedArtifact: number;
  skippedOther: number;
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
      // Nothing is running to cancel — Rust only accepts cancel from
      // queued/paused/in-flight, so offering it here was a dead button.
      // Settled rows are dropped with `removeDownloadTask` instead.
      return ["retry"];
    default:
      return [];
  }
}

/// Rows the "Gỡ khỏi danh sách" action may remove: exactly the states Rust
/// treats as settled. A `completed` row whose file is still on disk is refused
/// server-side, so the button can still report an error for that one case.
export function canRemoveTask(status: DownloadTaskDto["status"]): boolean {
  return status === "completed" || status === "failed" || status === "cancelled";
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

/// ETA from measured page progression. Guards the non-meaningful values:
/// null (no measurement yet), non-finite/negative (never show "∞"/NaN),
/// and sub-minute waits (hide instead of flickering "còn ~0s").
export function formatEtaSeconds(seconds: number | null | undefined): string | null {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds) || seconds < 60) {
    return null;
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `còn ~${minutes}p`;
  }
  return `còn ~${Math.floor(minutes / 60)}h${minutes % 60}p`;
}

/// Normalized polyline points for a 100x28 sparkline box; null when fewer
/// than two samples carry no shape information.
export function sparklinePoints(samples: number[]): string | null {
  if (samples.length < 2) {
    return null;
  }
  const max = Math.max(...samples, 1);
  const step = 100 / (samples.length - 1);
  return samples
    .map((value, index) => `${(index * step).toFixed(1)},${(28 - (value / max) * 26).toFixed(1)}`)
    .join(" ");
}
