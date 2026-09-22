import type { MangaSummary } from "../domain/manga";

export interface LibraryService {
  listManga(): Promise<MangaSummary[]>;
  listFavorites(): Promise<MangaSummary[]>;
  isFavorite(mangaId: string): Promise<boolean>;
  toggleFavorite(mangaId: string): Promise<boolean>;
  removeFromLibrary(mangaId: string): Promise<void>;
}
