export type SourceCapability = "search" | "resolveUrl" | "details" | "pages";

export type SourceStatus = "ready" | "disabled" | "unavailable";

export interface SourceViewDto {
  id: string;
  displayName: string;
  capabilities: SourceCapability[];
  status: SourceStatus;
  attribution: string | null;
  enabled: boolean;
  builtin: boolean;
  baseUrl: string | null;
  /** Comma-separated registered image CDN hosts (null = base host only). */
  imageHosts: string | null;
}

export interface RemoteMangaSummaryDto {
  sourceId: string;
  sourceMangaId: string;
  sourceDisplayName: string;
  title: string;
  author: string | null;
  coverKey: string | null;
  status: string;
}

export interface RemoteChapterDto {
  sourceChapterId: string;
  title: string;
  number: number | null;
  order: number;
  publishedAt: string | null;
}

export interface RemoteMangaDetailDto extends RemoteMangaSummaryDto {
  description: string | null;
  genres: string[];
  canonicalUrl: string | null;
  chapters: RemoteChapterDto[];
  savedMangaId: string | null;
  availableOffline: boolean;
}

export interface SearchSourceErrorDto {
  sourceId: string;
  code: string;
  message: string;
}

export interface SearchResultDto {
  items: RemoteMangaSummaryDto[];
  sourceErrors: SearchSourceErrorDto[];
  hasMore: boolean;
}

export interface SaveResultDto {
  mangaId: string;
  savedChapterCount: number;
  updated: boolean;
}

/** A source's own front page, split into the three Home rails. */
export interface HomeFeedItemDto extends RemoteMangaSummaryDto {
  /** Chapter label the source printed next to the card, e.g. "Chapter 3824". */
  latestChapter: string | null;
}

export interface HomeFeedDto {
  featured: HomeFeedItemDto[];
  newest: HomeFeedItemDto[];
  popular: HomeFeedItemDto[];
}
