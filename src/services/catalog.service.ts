import type { ChapterSummary, MangaDetail, MangaSource, MangaSummary } from "../domain/manga";

export interface CatalogSource extends MangaSource {
  enabled: boolean;
}

export interface CatalogService {
  search(query: string, sourceIds: string[]): Promise<MangaSummary[]>;
  resolveDemoUrl(url: string): Promise<MangaSummary | null>;
  getDemoExampleUrl(): Promise<string>;
  getManga(id: string): Promise<MangaDetail | null>;
  getChapters(mangaId: string): Promise<ChapterSummary[]>;
  listSources(): Promise<CatalogSource[]>;
  setSourceEnabled(sourceId: string, enabled: boolean): Promise<void>;
}

// Local-only matcher for the documented fictitious demo domain. Never a real fetch.
export const DEMO_SOURCE_PATTERN = /^https:\/\/demo\.manga1989\.example\/manga\/([a-z0-9-]+)\/?$/i;
