import * as desktop from "../shared/api/desktop";
import type { LocalLibraryService } from "./local-library.service";

/// Real adapter over Tauri IPC — the single production-mode library boundary.
export const localLibraryApi: LocalLibraryService = {
  listMangas: (args) => desktop.listLocalMangas(args),
  getManga: (mangaId) => desktop.getLocalManga(mangaId),
  setFavorite: (mangaId, favorite) => desktop.setMangaFavorite(mangaId, favorite),
  listFavorites: (args) => desktop.listFavoriteMangas(args),
  getProgress: (mangaId) => desktop.getReadingProgress(mangaId),
  saveProgress: (args) => desktop.saveReadingProgress(args),
};
