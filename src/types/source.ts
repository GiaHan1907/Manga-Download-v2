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
  /** Relative age the source printed, e.g. "37 phút trước". */
  latestTime: string | null;
}

export type RankingPeriod = "day" | "week" | "month";

export interface HomeFeedDto {
  featured: HomeFeedItemDto[];
  newest: HomeFeedItemDto[];
  popular: HomeFeedItemDto[];
}

/// One genre the source itself publishes. `slug` is what the source's own
/// catalog URL uses; the label is the source's wording, not ours.
export interface GenreTagDto {
  slug: string;
  label: string;
}

/// Closed set: "new" is the source's newest-first catalog.
export type BrowseStatus = "new" | "ongoing" | "completed";

export const BROWSE_STATUS_LABEL: Record<BrowseStatus, string> = {
  new: "Mới cập nhật",
  ongoing: "Đang tiến hành",
  completed: "Đã hoàn thành",
};

/// The six orderings the source's own catalog page publishes as `?sort=0..5`.
export type BrowseSort =
  | "newest-desc"
  | "newest-asc"
  | "updated-desc"
  | "updated-asc"
  | "views-desc"
  | "views-asc";

export const BROWSE_SORT_LABEL: Record<BrowseSort, string> = {
  "newest-desc": "Ngày đăng: mới nhất",
  "newest-asc": "Ngày đăng: cũ nhất",
  "updated-desc": "Cập nhật: mới nhất",
  "updated-asc": "Cập nhật: cũ nhất",
  "views-desc": "Lượt xem: cao nhất",
  "views-asc": "Lượt xem: thấp nhất",
};

/// Result of an explicit reachability check ("Kiểm tra") for one source.
/// `ok: false` is still a successful probe — it carries the source's own wire
/// code so the row can say what happened.
export interface SourceProbeDto {
  ok: boolean;
  latencyMs: number;
  resultCount: number;
  code: string | null;
  message: string | null;
}
