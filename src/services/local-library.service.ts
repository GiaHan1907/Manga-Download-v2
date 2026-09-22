import type {
  LocalMangaDetailDto,
  LocalMangaDto,
  PagedResult,
  ReadingProgressDto,
} from "../types/backend";

/// Local (SQLite-backed) library contract. Distinct from the demo
/// LibraryService: these records are real persistent rows, currently empty until
/// Phase 05 registers manga. Screens must render "empty" and "DB error" apart.
export interface LocalLibraryService {
  listMangas(args: { query?: string; limit: number; offset: number }): Promise<PagedResult<LocalMangaDto>>;
  getManga(mangaId: string): Promise<LocalMangaDetailDto>;
  setFavorite(mangaId: string, favorite: boolean): Promise<boolean>;
  listFavorites(args: { limit: number; offset: number }): Promise<PagedResult<LocalMangaDto>>;
  getProgress(mangaId: string): Promise<ReadingProgressDto | null>;
  saveProgress(args: { mangaId: string; chapterId: string; pageIndex: number }): Promise<ReadingProgressDto>;
}
