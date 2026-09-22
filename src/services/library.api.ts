import * as desktop from "../shared/api/desktop";
import { normalizeIpcError } from "../shared/api/desktop";
import type {
  DownloadedChapterDto,
  LibraryMangaDto,
  ReconcileReportDto,
  RemoveMangaFilesDto,
} from "../types/library";
import type { PagedResult } from "../types/backend";

/// Phase 07 production library boundary (SQLite-backed, aggregate queries).
export interface LibraryApi {
  list(args: {
    query?: string;
    sort?: string;
    filter?: string;
    limit: number;
    offset: number;
  }): Promise<PagedResult<LibraryMangaDto>>;
  downloadedChapters(mangaId: string): Promise<DownloadedChapterDto[]>;
  reconcile(): Promise<ReconcileReportDto>;
  removeChapter(chapterId: string): Promise<void>;
  removeMangaFiles(mangaId: string): Promise<RemoveMangaFilesDto>;
}

function rethrow(err: unknown): never {
  throw new Error(normalizeIpcError(err).message);
}

export const libraryApi: LibraryApi = {
  list: (args) => desktop.libraryList(args).catch(rethrow),
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
};
