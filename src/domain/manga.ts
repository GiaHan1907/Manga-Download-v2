export type MangaStatus = "ongoing" | "completed" | "unknown";

export interface MangaSummary {
  id: string;
  sourceId: string;
  sourceMangaId: string;
  title: string;
  author?: string;
  coverAsset?: string;
  genres: string[];
  status: MangaStatus;
}

export interface ChapterSummary {
  id: string;
  mangaId: string;
  title: string;
  number?: number;
  pageCount?: number;
  hasSamplePages: boolean;
}

export interface MangaDetail extends MangaSummary {
  description: string;
  chapters: ChapterSummary[];
}

export interface MangaSource {
  id: string;
  label: string;
  demo: true;
  enabledByDefault: boolean;
}

export function formatChapterLabel(chapter: ChapterSummary): string {
  return chapter.number !== undefined ? `Chapter ${chapter.number} — ${chapter.title}` : chapter.title;
}
