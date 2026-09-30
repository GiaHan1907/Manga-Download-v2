import * as desktop from "../shared/api/desktop";
import type { ReadHistoryDto, RecentSearchDto } from "../types/backend";

export interface HistoryService {
  recordSearch(query: string): Promise<RecentSearchDto[]>;
  listSearches(): Promise<RecentSearchDto[]>;
  removeSearch(query: string): Promise<RecentSearchDto[]>;
  clearSearches(): Promise<number>;
  listRead(args: { mangaId?: string; limit: number; offset: number }): Promise<{
    items: ReadHistoryDto[];
    total: number;
    limit: number;
    offset: number;
  }>;
}

/// Discover Phase 2 boundary: the recent-query list and the reader's own
/// history. Record/remove return the authoritative list from SQLite.
export const historyApi: HistoryService = {
  recordSearch: (query) => desktop.recordSearchHistory(query),
  listSearches: () => desktop.listSearchHistory(),
  removeSearch: (query) => desktop.removeSearchHistory(query),
  clearSearches: () => desktop.clearSearchHistory(),
  listRead: (args) => desktop.listReadHistory(args),
};
