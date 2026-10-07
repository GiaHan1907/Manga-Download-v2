// DTO mirrors of the Rust command payloads (docs/IPC_CONTRACT.md). Field names
// are the serialized camelCase forms; keep both sides in sync when editing.

export type PagedResult<T> = {
  items: T[];
  total: number;
  limit: number;
  offset: number;
};

/// Feature plan F4 — per-chapter read history (Rust serde camelCase).
export type ReadHistoryDto = {
  mangaId: string;
  mangaTitle: string;
  chapterId: string;
  chapterTitle: string;
  chapterNumber: number | null;
  readAt: string;
};

/// Discover Phase 2 — one recent search query (Rust serde camelCase). Only the
/// text and its timestamp exist; results are never persisted.
export type RecentSearchDto = {
  query: string;
  searchedAt: string;
};

export type LocalMangaDto = {
  id: string;
  sourceId: string;
  sourceMangaId: string;
  title: string;
  author: string | null;
  description: string | null;
  status: "ongoing" | "completed" | "unknown";
  /** Internal cover path key for get_manga_cover (never a URL). */
  coverKey: string | null;
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
  /// Chapter number, joined server-side so the queue can be read in chapter
  /// order. Optional because test fixtures and the demo service predate it.
  chapterNumber?: number | null;
  createdAt: string;
  updatedAt: string;
};

export type AppSettingsDto = {
  defaultReaderMode: "single" | "double" | "vertical";
  defaultReadingDirection: "ltr" | "rtl";
  chapterConcurrency: number;
  imageConcurrency: number;
  outputFormat: "cbz" | "folder" | "pdf";
  notificationsEnabled: boolean;
  closeToTray: boolean;
  /** Feature plan F1: global watcher default (per-manga override in Library). */
  watchEnabled: boolean;
};

/// Phase 08 — Home overview aggregates (library_stats).
export type LibraryStatsDto = {
  savedMangas: number;
  downloadedChapters: number;
  localSizeBytes: number;
  favorites: number;
  /** Rust sends these; older dev fixtures omit them, so treat as optional. */
  totalChapters?: number;
  downloadedMangas?: number;
  reading?: number;
  completed?: number;
  paused?: number;
  missing?: number;
};

/// Phase 08 — storage accounting (approximate by contract; no paths).
export type StorageStatusDto = {
  libraryBytes: number;
  cacheBytes: number;
  stagingDirs: number;
  activeTasks: number;
  rootAvailable: boolean;
  approximate: boolean;
  customRoot: boolean;
  rootLinkOk: boolean;
  /** Real volume capacity of the drive holding the library. Absent off
   *  Windows or when the OS call fails — the UI drops the bar rather than
   *  showing an invented quota. */
  volumeTotalBytes?: number | null;
  volumeFreeBytes?: number | null;
};

export type DownloadRootDto = {
  isCustom: boolean;
  label: string;
  linkOk: boolean;
};

export type CacheCleanupDto = {
  removedDirs: number;
  freedBytes: number;
  keptActiveDirs: number;
};

export type SettingsPatchDto = Partial<AppSettingsDto>;
