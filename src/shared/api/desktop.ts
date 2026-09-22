import { invoke } from "@tauri-apps/api/core";
import type { AppError, AppHealth, AppInfo } from "./contracts";
import type {
  AppSettingsDto,
  DownloadTaskDto,
  LocalMangaDetailDto,
  LocalMangaDto,
  PagedResult,
  ReadingProgressDto,
  SettingsPatchDto,
} from "../../types/backend";

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

export function normalizeIpcError(error: unknown): AppError {
  if (typeof error === "object" && error !== null) {
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

function isAppErrorCode(value: unknown): value is AppError["code"] {
  return (
    value === "DATABASE_INIT_FAILED" ||
    value === "VALIDATION_ERROR" ||
    value === "NOT_FOUND" ||
    value === "DB_CONFLICT" ||
    value === "UNSUPPORTED_IN_THIS_PHASE" ||
    value === "INTERNAL_ERROR"
  );
}
