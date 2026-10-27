import * as desktop from "../shared/api/desktop";
import type {
  DownloadedChapterDto,
  LibraryGenreDto,
  LibraryMangaDto,
  ReconcileReportDto,
  RemoveMangaDto,
  RemoveMangaFilesDto,
  ImportResultDto,
} from "../types/library";
import type { PagedResult } from "../types/backend";
import { rethrowIpc } from "../shared/api/errors";

/// Phase 07 production library boundary (SQLite-backed, aggregate queries).
export interface LibraryApi {
  list(args: {
    query?: string;
    sort?: string;
    filter?: string;
    genre?: string;
    limit: number;
    offset: number;
  }): Promise<PagedResult<LibraryMangaDto>>;
  genres(): Promise<LibraryGenreDto[]>;
  downloadedChapters(mangaId: string): Promise<DownloadedChapterDto[]>;
  reconcile(): Promise<ReconcileReportDto>;
  removeChapter(chapterId: string): Promise<void>;
  removeMangaFiles(mangaId: string): Promise<RemoveMangaFilesDto>;
  removeManga(args: { mangaId: string; deleteFiles: boolean }): Promise<RemoveMangaDto>;
  importCbz(args: {
    mangaId: string;
    chapterNumber?: number;
    chapterTitle?: string;
  }): Promise<ImportResultDto | null>;
  importCbzPath(args: { mangaId: string; path: string }): Promise<ImportResultDto>;
  getWatch(mangaId: string): Promise<desktop.WatchStateDto>;
  setWatch(args: { mangaId: string; enabled: boolean | null }): Promise<void>;
  listAlerts(mangaId: string): Promise<desktop.ChapterAlertDto[]>;
  ackAlerts(mangaId: string): Promise<number>;
}

export const libraryApi: LibraryApi = {
  list: (args) => desktop.libraryList(args).catch(rethrowIpc),
  genres: () => desktop.libraryGenres().catch(rethrowIpc),
  downloadedChapters: async (mangaId) => {
    try {
      const result = await desktop.libraryDownloadedChapters(mangaId);
      return result.chapters;
    } catch (err) {
      rethrowIpc(err);
    }
  },
  reconcile: () => desktop.libraryReconcile().catch(rethrowIpc),
  removeChapter: (chapterId) => desktop.libraryRemoveChapter(chapterId).catch(rethrowIpc),
  removeMangaFiles: (mangaId) => desktop.libraryRemoveMangaFiles(mangaId).catch(rethrowIpc),
  removeManga: (args) => desktop.libraryRemoveManga(args).catch(rethrowIpc),
  importCbz: (args) => desktop.libraryImportCbz(args).catch(rethrowIpc),
  importCbzPath: (args) => desktop.libraryImportCbzPath(args).catch(rethrowIpc),
  getWatch: (mangaId) => desktop.libraryGetWatch(mangaId).catch(rethrowIpc),
  setWatch: (args) => desktop.librarySetWatch(args).catch(rethrowIpc),
  listAlerts: (mangaId) => desktop.libraryListAlerts(mangaId).catch(rethrowIpc),
  ackAlerts: (mangaId) => desktop.libraryAckAlerts(mangaId).catch(rethrowIpc),
};
