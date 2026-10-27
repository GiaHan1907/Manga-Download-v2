import * as desktop from "../shared/api/desktop";
import { rethrowIpc } from "../shared/api/errors";

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

export const readerApi: ReaderService = {
  openChapter: (chapterId) => desktop.readerOpenChapter(chapterId).catch(rethrowIpc),
  getPageBytes: async (chapterId, pageIndex) => {
    try {
      return await desktop.readerGetPageBytes(chapterId, pageIndex);
    } catch (err) {
      rethrowIpc(err);
    }
  },
  getProgress: (mangaId) => desktop.getReadingProgress(mangaId).catch(rethrowIpc),
  saveProgress: (args) => desktop.saveReadingProgress(args).catch(rethrowIpc),
};
