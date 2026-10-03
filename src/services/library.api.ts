import * as desktop from "../shared/api/desktop";
import { normalizeIpcError } from "../shared/api/desktop";
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

function rethrow(err: unknown): never {
  throw new Error(normalizeIpcError(err).message);
}

export const libraryApi: LibraryApi = {
  list: (args) => desktop.libraryList(args).catch(rethrow),
  genres: () => desktop.libraryGenres().catch(rethrow),
  downloadedChapters: async (mangaId) => {
    try {
      const result = await desktop.libraryDownloadedChapters(mangaId);
      return result.chapters;
    } catch (err) {
      rethrow(err);
    }
  },
  reconcile: () => desktop.libraryReconcile().catch(rethrow),
  removeChapter: (chapterId) => desktop.libraryRemoveChapter(chapterId).catch(rethrow),
  removeMangaFiles: (mangaId) => desktop.libraryRemoveMangaFiles(mangaId).catch(rethrow),
  removeManga: (args) => desktop.libraryRemoveManga(args).catch(rethrow),
  importCbz: (args) => desktop.libraryImportCbz(args).catch(rethrow),
  importCbzPath: (args) => desktop.libraryImportCbzPath(args).catch(rethrow),
  getWatch: (mangaId) => desktop.libraryGetWatch(mangaId).catch(rethrow),
  setWatch: (args) => desktop.librarySetWatch(args).catch(rethrow),
  listAlerts: (mangaId) => desktop.libraryListAlerts(mangaId).catch(rethrow),
  ackAlerts: (mangaId) => desktop.libraryAckAlerts(mangaId).catch(rethrow),
};
