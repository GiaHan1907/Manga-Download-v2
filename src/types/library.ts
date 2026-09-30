import type { ReadingProgressDto } from "./backend";

/// Phase 07 DTO mirrors (Rust serde camelCase). Reader manifests carry no
/// paths: pages are (chapterId, index) addressed and bytes come per page.

export type LibrarySort = "lastRead" | "recentDownload" | "title" | "chapters";
export type LibraryFilter =
  | "all"
  | "downloaded"
  | "partial"
  | "favorite"
  | "alerts"
  | "reading"
  | "completed"
  | "missing";

/// One genre present in the local library. The count is OUR row count, so it
/// can be displayed; it is never the source's story total.
export type LibraryGenreDto = {
  genre: string;
  mangaCount: number;
};

export type LibraryProgressDto = {
  chapterId: string;
  /** The saved chapter's own title, for the "Tiếp tục đọc" rail. */
  chapterTitle?: string | null;
  pageIndex: number;
  pageTotal: number | null;
  updatedAt: string;
};

export type LibraryMangaDto = {
  id: string;
  title: string;
  /** The source's own synopsis; the Library's highlight panel quotes it. */
  description?: string | null;
  author: string | null;
  status: "ongoing" | "completed" | "unknown";
  /** Genres saved with the story. Rust always sends the field; dev fixtures
   *  may omit it, so treat undefined as "no genres known". */
  genres?: string[];
  /** Cover trio for get_manga_cover (path key only — never a URL). */
  sourceId: string;
  sourceMangaId: string;
  coverKey: string | null;
  favorite: boolean;
  downloadedChapters: number;
  totalChapters: number;
  localSizeBytes: number | null;
  lastReadAt: string | null;
  lastDownloadAt: string | null;
  progress: LibraryProgressDto | null;
  hasMissing: boolean;
  /** Feature plan F1: unacknowledged new-chapter alerts (Rust always sends;
   *  optional in TS so demo fixtures may omit it). */
  newAlerts?: number;
};

export type DownloadedChapterDto = {
  id: string;
  title: string;
  number: number | null;
  pageCount: number | null;
  storageFormat: "cbz" | "folder" | null;
  sizeBytes: number | null;
  artifactOk: boolean;
  updatedAt: string;
};

export type ReconcileReportDto = {
  checked: number;
  healthy: number;
  missing: number;
  corrupted: number;
};

export type RemoveMangaFilesDto = {
  removed: string[];
  failed: string[];
};

export type ReaderPageDto = {
  index: number;
  width: number | null;
  height: number | null;
};

export type ReaderManifestDto = {
  chapterId: string;
  mangaId: string;
  mangaTitle: string;
  chapterTitle: string;
  format: "cbz" | "folder";
  pageCount: number;
  pages: ReaderPageDto[];
  prevChapterId: string | null;
  nextChapterId: string | null;
  startPage: number;
  startScrollOffset: number | null;
};

/// Feature plan F3b — result of importing an external CBZ (path-free: only
/// generated ids + archive metadata; `null` from the command means cancelled).
export type ImportResultDto = {
  mangaId: string;
  chapterId: string;
  number: number;
  title: string;
  pageCount: number;
  sizeBytes: number;
};

export type { ReadingProgressDto };
