import type { ReaderPage, ReadingProgress } from "../domain/reader";

export interface ReaderService {
  getPages(chapterId: string): Promise<ReaderPage[]>;
  getProgress(mangaId: string): Promise<ReadingProgress | null>;
  listProgress(): Promise<ReadingProgress[]>;
  setProgress(progress: ReadingProgress): Promise<void>;
}
