import * as desktop from "../shared/api/desktop";
import type {
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
  setSourceEnabled(args: { sourceId: string; enabled: boolean }): Promise<boolean>;
  search(args: {
    query: string;
    sourceIds?: string[];
    page?: number;
  }): Promise<SearchResultDto>;
  resolveUrl(url: string): Promise<RemoteMangaDetailDto>;
  remoteDetail(args: {
    sourceId: string;
    sourceMangaId: string;
  }): Promise<RemoteMangaDetailDto>;
  remoteChapters(args: { sourceId: string; sourceMangaId: string }): Promise<RemoteChapterDto[]>;
  save(args: { sourceId: string; sourceMangaId: string }): Promise<SaveResultDto>;
}

/// Real adapter over Tauri IPC — production discovery boundary (Phase 05).
export const sourceApi: SourceService = {
  listSources: () => desktop.listSupportedSources(),
  addSource: (args) => desktop.addMangaSource(args),
  removeSource: (sourceId) => desktop.removeMangaSource(sourceId),
  setSourceEnabled: (args) => desktop.setSourceEnabled(args),
  search: (args) => desktop.searchManga(args),
  resolveUrl: (url) => desktop.resolveMangaUrl(url),
  remoteDetail: (args) => desktop.getRemoteMangaDetails(args),
  remoteChapters: (args) => desktop.getRemoteChapters(args),
  save: (args) => desktop.saveDiscoveredManga(args),
};
