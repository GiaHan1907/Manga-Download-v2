import type { ReadingProgressDto } from "./backend";

/// Phase 07 DTO mirrors (Rust serde camelCase). Reader manifests carry no
/// paths: pages are (chapterId, index) addressed and bytes come per page.

export type LibrarySort = "lastRead" | "recentDownload" | "title" | "chapters";
export type LibraryFilter = "all" | "downloaded" | "partial" | "favorite";

export type LibraryProgressDto = {
  chapterId: string;
  pageIndex: number;
  pageTotal: number | null;
  updatedAt: string;
};

export type LibraryMangaDto = {
  id: string;
  title: string;
  author: string | null;
  status: "ongoing" | "completed" | "unknown";
  favorite: boolean;
  downloadedChapters: number;
  totalChapters: number;
  localSizeBytes: number | null;
  lastReadAt: string | null;
  lastDownloadAt: string | null;
  progress: LibraryProgressDto | null;
  hasMissing: boolean;
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

export type { ReadingProgressDto };
