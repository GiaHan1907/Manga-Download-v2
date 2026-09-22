import type { DemoDownloadTask, DownloadAction } from "../domain/download";

export interface QueueResult {
  added: number;
  skipped: number;
}

export class InvalidTransitionError extends Error {
  constructor(taskId: string, action: DownloadAction) {
    super(`Không thể thực hiện "${action}" cho tác vụ ${taskId} ở trạng thái hiện tại.`);
    this.name = "InvalidTransitionError";
  }
}

export interface DownloadService {
  listTasks(): Promise<DemoDownloadTask[]>;
  queueChapters(chapterIds: string[]): Promise<QueueResult>;
  transition(taskId: string, action: DownloadAction): Promise<void>;
}
