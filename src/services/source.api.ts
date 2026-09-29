import * as desktop from "../shared/api/desktop";
import type {
  HomeFeedDto,
  HomeFeedItemDto,
  RankingPeriod,
  RemoteChapterDto,
  RemoteMangaDetailDto,
  SaveResultDto,
  SearchResultDto,
  SourceViewDto,
} from "../types/source";

export interface SourceService {
  listSources(): Promise<SourceViewDto[]>;
  addSource(args: {
    displayName: string;
    baseUrl: string;
    imageHosts?: string[];
  }): Promise<SourceViewDto>;
  removeSource(sourceId: string): Promise<void>;
  updateSource(args: {
    sourceId: string;
    displayName: string;
    baseUrl: string;
    imageHosts?: string[];
  }): Promise<SourceViewDto>;
  setSourceEnabled(args: { sourceId: string; enabled: boolean }): Promise<boolean>;
  search(args: {
    query: string;
    sourceIds?: string[];
    page?: number;
  }): Promise<SearchResultDto>;
  homeFeed(sourceId: string): Promise<HomeFeedDto>;
  ranking(args: {
    sourceId: string;
    period: RankingPeriod;
  }): Promise<HomeFeedItemDto[]>;
  resolveUrl(url: string): Promise<RemoteMangaDetailDto>;
  remoteDetail(args: {
    sourceId: string;
    sourceMangaId: string;
  }): Promise<RemoteMangaDetailDto>;
  remoteChapters(args: { sourceId: string; sourceMangaId: string }): Promise<RemoteChapterDto[]>;
  save(args: { sourceId: string; sourceMangaId: string }): Promise<SaveResultDto>;
  fetchCover(args: {
    sourceId: string;
    sourceMangaId: string;
    coverKey: string;
  }): Promise<Uint8Array>;
}

/// Real adapter over Tauri IPC — production discovery boundary (Phase 05).
export const sourceApi: SourceService = {
  listSources: () => desktop.listSupportedSources(),
  addSource: (args) => desktop.addMangaSource(args),
  removeSource: (sourceId) => desktop.removeMangaSource(sourceId),
  updateSource: (args) => desktop.updateMangaSource(args),
  setSourceEnabled: (args) => desktop.setSourceEnabled(args),
  search: (args) => desktop.searchManga(args),
  homeFeed: (sourceId) => desktop.getHomeFeed(sourceId),
  ranking: (args) => desktop.getHomeRanking(args.sourceId, args.period),
  resolveUrl: (url) => desktop.resolveMangaUrl(url),
  remoteDetail: (args) => desktop.getRemoteMangaDetails(args),
  remoteChapters: (args) => desktop.getRemoteChapters(args),
  save: (args) => desktop.saveDiscoveredManga(args),
  fetchCover: (args) => desktop.getMangaCover(args),
};
