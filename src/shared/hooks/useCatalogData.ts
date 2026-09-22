import type { ChapterSummary, MangaSummary } from "../../domain/manga";
import { services } from "../../services/provider";
import { useAsyncData, type AsyncData } from "./useAsyncData";
import { useServicesVersion } from "./useServices";

export interface CatalogMaps {
  mangaById: Map<string, MangaSummary>;
  chapterById: Map<string, ChapterSummary>;
  allManga: MangaSummary[];
  allChapters: ChapterSummary[];
}

export function useCatalogData(): AsyncData<CatalogMaps> {
  const version = useServicesVersion();
  return useAsyncData<CatalogMaps>(async () => {
    const allManga = await services.catalog.search("", []);
    const chapterLists = await Promise.all(allManga.map((manga) => services.catalog.getChapters(manga.id)));
    const allChapters = chapterLists.flat();
    return {
      mangaById: new Map(allManga.map((manga) => [manga.id, manga])),
      chapterById: new Map(allChapters.map((chapter) => [chapter.id, chapter])),
      allManga,
      allChapters,
    };
  }, [version]);
}
