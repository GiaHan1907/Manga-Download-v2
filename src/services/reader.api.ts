import * as desktop from "../shared/api/desktop";
import { normalizeIpcError } from "../shared/api/desktop";

/// Phase 07 production reader boundary. Pages are addressed by
/// (chapterId, pageIndex); bytes arrive per page as a Uint8Array that the
/// UI turns into a short-lived blob URL. No path ever crosses the IPC line.
export interface ReaderService {
  openChapter(chapterId: string): ReturnType<typeof desktop.readerOpenChapter>;
  getPageBytes(chapterId: string, pageIndex: number): Promise<Uint8Array>;
  getProgress(mangaId: string): ReturnType<typeof desktop.getReadingProgress>;
  saveProgress(args: {
    mangaId: string;
    chapterId: string;
    pageIndex: number;
    scrollOffset?: number;
    pageTotal?: number;
  }): ReturnType<typeof desktop.saveReadingProgress>;
}

function rethrow(err: unknown): never {
  throw new Error(normalizeIpcError(err).message);
}

export const readerApi: ReaderService = {
  openChapter: (chapterId) => desktop.readerOpenChapter(chapterId).catch(rethrow),
  getPageBytes: async (chapterId, pageIndex) => {
    try {
      return await desktop.readerGetPageBytes(chapterId, pageIndex);
    } catch (err) {
      rethrow(err);
    }
  },
  getProgress: (mangaId) => desktop.getReadingProgress(mangaId).catch(rethrow),
  saveProgress: (args) => desktop.saveReadingProgress(args).catch(rethrow),
};
