// DTO mirrors of the Rust command payloads (docs/IPC_CONTRACT.md). Field names
// are the serialized camelCase forms; keep both sides in sync when editing.

export type PagedResult<T> = {
  items: T[];
  total: number;
  limit: number;
  offset: number;
};

export type LocalMangaDto = {
  id: string;
  sourceId: string;
  sourceMangaId: string;
  title: string;
  author: string | null;
  description: string | null;
  status: "ongoing" | "completed" | "unknown";
  chapterCount: number;
  favorite: boolean;
  updatedAt: string;
};

export type LocalChapterDto = {
  id: string;
  mangaId: string;
  sourceChapterId: string;
  title: string;
  number: number | null;
  pageCount: number | null;
  downloaded: boolean;
  outputFormat: "cbz" | "folder" | null;
};

export type LocalMangaDetailDto = LocalMangaDto & {
  chapters: LocalChapterDto[];
};

export type ReadingProgressDto = {
  mangaId: string;
  chapterId: string;
  pageIndex: number;
  updatedAt: string;
};

export type DownloadTaskDto = {
  id: string;
  chapterId: string;
  status:
    | "queued"
    | "resolving"
    | "downloading"
    | "pausing"
    | "paused"
    | "verifying"
    | "exporting"
    | "completed"
    | "failed"
    | "cancelled";
  targetFormat: "cbz" | "folder" | "pdf";
  priority: number;
  completedPages: number;
  totalPages: number;
  attempts: number;
  nextRetryAt: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  revision: number;
  finishedAt: string | null;
  outputRelpath: string | null;
  chapterTitle: string | null;
  mangaTitle: string | null;
  createdAt: string;
  updatedAt: string;
};

export type AppSettingsDto = {
  defaultReaderMode: "single" | "double" | "vertical";
  defaultReadingDirection: "ltr" | "rtl";
  chapterConcurrency: number;
  imageConcurrency: number;
  outputFormat: "cbz" | "folder" | "pdf";
};

export type SettingsPatchDto = Partial<AppSettingsDto>;
