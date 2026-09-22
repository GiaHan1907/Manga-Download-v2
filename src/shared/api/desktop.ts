import { invoke } from "@tauri-apps/api/core";
import type { AppError, AppErrorCode, AppHealth, AppInfo } from "./contracts";
import type {
  AppSettingsDto,
  DownloadTaskDto,
  LocalMangaDetailDto,
  LocalMangaDto,
  PagedResult,
  ReadingProgressDto,
  SettingsPatchDto,
} from "../../types/backend";
import type {
  RemoteChapterDto,
  RemoteMangaDetailDto,
  SaveResultDto,
  SearchResultDto,
  SourceViewDto,
} from "../../types/source";

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_get_info");
}

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health_check");
}

export async function getBackendSettings(): Promise<AppSettingsDto> {
  return invoke<AppSettingsDto>("app_get_settings");
}

export async function updateBackendSettings(input: SettingsPatchDto): Promise<AppSettingsDto> {
  return invoke<AppSettingsDto>("app_update_settings", { input });
}

export async function resetBackendSettings(): Promise<AppSettingsDto> {
  return invoke<AppSettingsDto>("app_reset_settings");
}

export async function listLocalMangas(args: {
  query?: string;
  limit: number;
  offset: number;
}): Promise<PagedResult<LocalMangaDto>> {
  return invoke<PagedResult<LocalMangaDto>>("app_list_local_mangas", args);
}

export async function getLocalManga(mangaId: string): Promise<LocalMangaDetailDto> {
  return invoke<LocalMangaDetailDto>("app_get_local_manga", { mangaId });
}

export async function setMangaFavorite(mangaId: string, favorite: boolean): Promise<boolean> {
  return invoke<boolean>("app_set_manga_favorite", { mangaId, favorite });
}

export async function listFavoriteMangas(args: {
  limit: number;
  offset: number;
}): Promise<PagedResult<LocalMangaDto>> {
  return invoke<PagedResult<LocalMangaDto>>("app_list_favorite_mangas", args);
}

export async function getReadingProgress(mangaId: string): Promise<ReadingProgressDto | null> {
  return invoke<ReadingProgressDto | null>("app_get_reading_progress", { mangaId });
}

export async function saveReadingProgress(args: {
  mangaId: string;
  chapterId: string;
  pageIndex: number;
}): Promise<ReadingProgressDto> {
  return invoke<ReadingProgressDto>("app_save_reading_progress", args);
}

export async function listLocalDownloadTasks(args: {
  status?: string;
  limit: number;
  offset: number;
}): Promise<PagedResult<DownloadTaskDto>> {
  return invoke<PagedResult<DownloadTaskDto>>("app_list_local_download_tasks", args);
}

// --- Phase 05: source engine & discovery (narrow, registry-backed commands) ---

export async function listSupportedSources(): Promise<SourceViewDto[]> {
  return invoke<SourceViewDto[]>("list_supported_sources");
}

export async function addMangaSource(args: {
  displayName: string;
  baseUrl: string;
}): Promise<SourceViewDto> {
  return invoke<SourceViewDto>("add_manga_source", args);
}

export async function removeMangaSource(sourceId: string): Promise<void> {
  return invoke<void>("remove_manga_source", { sourceId });
}

export async function setSourceEnabled(args: {
  sourceId: string;
  enabled: boolean;
}): Promise<boolean> {
  return invoke<boolean>("set_source_enabled", args);
}

export async function searchManga(args: {
  query: string;
  sourceIds?: string[];
  page?: number;
}): Promise<SearchResultDto> {
  return invoke<SearchResultDto>("search_manga", args);
}

export async function resolveMangaUrl(url: string): Promise<RemoteMangaDetailDto> {
  return invoke<RemoteMangaDetailDto>("resolve_manga_url", { url });
}

export async function getRemoteMangaDetails(args: {
  sourceId: string;
  sourceMangaId: string;
}): Promise<RemoteMangaDetailDto> {
  return invoke<RemoteMangaDetailDto>("get_remote_manga_details", args);
}

export async function getRemoteChapters(args: {
  sourceId: string;
  sourceMangaId: string;
}): Promise<RemoteChapterDto[]> {
  return invoke<RemoteChapterDto[]>("get_remote_chapters", args);
}

export async function saveDiscoveredManga(args: {
  sourceId: string;
  sourceMangaId: string;
}): Promise<SaveResultDto> {
  return invoke<SaveResultDto>("save_discovered_manga", args);
}

export function normalizeIpcError(error: unknown): AppError {  if (typeof error === "object" && error !== null) {
    const candidate = error as Record<string, unknown>;
    const code = isAppErrorCode(candidate.code) ? candidate.code : "INTERNAL_ERROR";
    const message =
      typeof candidate.message === "string" ? candidate.message : "Unknown IPC error";
    const retryable = typeof candidate.retryable === "boolean" ? candidate.retryable : false;
    return { code, message, retryable };
  }
  return {
    code: "INTERNAL_ERROR",
    message: typeof error === "string" ? error : "Unknown IPC error",
    retryable: false,
  };
}

const APP_ERROR_CODES: ReadonlySet<string> = new Set<AppErrorCode>([
  "DATABASE_INIT_FAILED",
  "VALIDATION_ERROR",
  "NOT_FOUND",
  "DB_CONFLICT",
  "UNSUPPORTED_IN_THIS_PHASE",
  "INTERNAL_ERROR",
  "INVALID_URL",
  "UNSUPPORTED_SOURCE",
  "SOURCE_DISABLED",
  "UNSUPPORTED_OPERATION",
  "SOURCE_NOT_FOUND",
  "SOURCE_RATE_LIMITED",
  "SOURCE_NETWORK",
  "SOURCE_PARSE_ERROR",
  "SOURCE_UPSTREAM",
]);

function isAppErrorCode(value: unknown): value is AppError["code"] {
  return typeof value === "string" && APP_ERROR_CODES.has(value);
}
