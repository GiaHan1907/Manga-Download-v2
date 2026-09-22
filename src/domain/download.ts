export type DownloadStatus = "queued" | "paused" | "failed" | "completed" | "cancelled";

export type DownloadAction = "pause" | "resume" | "retry" | "cancel";

export interface DemoDownloadTask {
  id: string;
  mangaId: string;
  chapterId: string;
  status: DownloadStatus;
  completedPages: number;
  totalPages: number;
  createdAt: string;
}

const TRANSITIONS: Record<DownloadStatus, Partial<Record<DownloadAction, DownloadStatus>>> = {
  queued: { pause: "paused", cancel: "cancelled" },
  paused: { resume: "queued", cancel: "cancelled" },
  failed: { retry: "queued", cancel: "cancelled" },
  completed: {},
  cancelled: {},
};

export function applyDownloadAction(status: DownloadStatus, action: DownloadAction): DownloadStatus | null {
  return TRANSITIONS[status][action] ?? null;
}
