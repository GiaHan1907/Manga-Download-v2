import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppError, AppErrorCode, AppHealth, AppInfo } from "./contracts";
import type {
  AppSettingsDto,
  CacheCleanupDto,
  DownloadTaskDto,
  LibraryStatsDto,
  LocalMangaDetailDto,
  LocalMangaDto,
  PagedResult,
  ReadHistoryDto,
  ReadingProgressDto,
  SettingsPatchDto,
  StorageStatusDto,
} from "../../types/backend";
import type { QueueResultDto, DownloadTaskEvent, ClearHistoryDto } from "../../types/downloads";
import type {
  DownloadedChapterDto,
  LibraryMangaDto,
  ReconcileReportDto,
  ReaderManifestDto,
  RemoveMangaFilesDto,
  ImportResultDto,
} from "../../types/library";
import { DOWNLOAD_EVENT } from "../../types/downloads";
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

/// Feature plan F4 — read history (newest first; omit mangaId for the global stream).
export async function listReadHistory(args: {
  mangaId?: string;
  limit: number;
  offset: number;
}): Promise<PagedResult<ReadHistoryDto>> {
  return invoke<PagedResult<ReadHistoryDto>>("app_list_read_history", args);
}

export async function clearReadHistory(mangaId: string): Promise<number> {
  return invoke<number>("app_clear_read_history", { mangaId });
}

export async function saveReadingProgress(args: {
  mangaId: string;
  chapterId: string;
  pageIndex: number;
  scrollOffset?: number;
  pageTotal?: number;
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
  /** Optional exact hostnames that may serve this source's chapter images
   * (max 4, validated by Rust; requests to any other host stay blocked). */
  imageHosts?: string[];
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

/// Cover bytes (Rust `ipc::Response`), normalized to Uint8Array like reader
/// pages. Only opaque ids/keys are sent — no URL crosses this boundary.
export async function getMangaCover(args: {
  sourceId: string;
  sourceMangaId: string;
  coverKey: string;
}): Promise<Uint8Array> {
  const response = await invoke<ArrayBuffer | Uint8Array | number[]>("get_manga_cover", args);
  if (response instanceof Uint8Array) {
    return response;
  }
  if (response instanceof ArrayBuffer) {
    return new Uint8Array(response);
  }
  return Uint8Array.from(response);
}

// --- Phase 06: download engine (chapter ids + actions only; no URLs/paths) ---

export async function queueChapterDownloads(args: {
  chapterIds: string[];
  format?: string;
  priority?: number;
}): Promise<QueueResultDto> {
  return invoke<QueueResultDto>("queue_chapter_downloads", args);
}

export async function getDownloadTask(taskId: string): Promise<DownloadTaskDto> {
  return invoke<DownloadTaskDto>("get_download_task", { taskId });
}

export async function pauseDownload(taskId: string): Promise<DownloadTaskDto> {
  return invoke<DownloadTaskDto>("pause_download", { taskId });
}

export async function resumeDownload(taskId: string): Promise<DownloadTaskDto> {
  return invoke<DownloadTaskDto>("resume_download", { taskId });
}

export async function cancelDownload(taskId: string): Promise<DownloadTaskDto> {
  return invoke<DownloadTaskDto>("cancel_download", { taskId });
}

export async function retryDownload(taskId: string): Promise<DownloadTaskDto> {
  return invoke<DownloadTaskDto>("retry_download", { taskId });
}

export async function setDownloadPriority(args: {
  taskId: string;
  priority: number;
}): Promise<DownloadTaskDto> {
  return invoke<DownloadTaskDto>("set_download_priority", args);
}

/// Feature plan F2: purge terminal history rows (Rust decides what is safe —
/// completed-with-artifact rows are never deleted there).
export async function clearDownloadHistory(args: {
  statuses?: string[];
  olderThanDays?: number;
}): Promise<ClearHistoryDto> {
  return invoke<ClearHistoryDto>("clear_download_history", {
    statuses: args.statuses,
    olderThanDays: args.olderThanDays,
  });
}

/// Feature plan F5: bulk requeue of `failed` tasks (cap server-side, 200).
export async function retryAllFailedDownloads(): Promise<number> {
  return invoke<number>("retry_all_failed_downloads");
}

/// Feature plan F5: request cancel for every unfinished task of a manga.
/// Returns the accepted count; final states arrive via task events.
export async function cancelDownloadsForManga(mangaId: string): Promise<number> {
  return invoke<number>("cancel_downloads_for_manga", { mangaId });
}

/// Paste-a-chapter-link quick download: the URL is one the USER provided
/// (same documented exception as resolveMangaUrl); Rust resolves it against
/// registered adapters, saves the manga if needed and queues through the
/// standard gate. Response is the normal path-free QueueResultDto.
export async function queueChapterDownloadByUrl(args: {
  url: string;
  format?: string;
}): Promise<QueueResultDto> {
  return invoke<QueueResultDto>("queue_chapter_download_by_url", args);
}

/// Open Explorer at the download location: with a completed task's id the
/// artifact is SELECTED in its folder; with no id the library root opens.
/// Paths never cross IPC in either direction — this is an action, not a getter.
export async function revealDownload(args: { taskId?: string } = {}): Promise<void> {
  await invoke<void>("reveal_download", args);
}

/// Subscribe to coalesced task updates. Returns an unsubscribe function that
/// MUST be called on teardown (design §8.2 listener cleanup).
export async function subscribeDownloadTask(
  handler: (event: DownloadTaskEvent) => void,
): Promise<UnlistenFn> {
  return listen<DownloadTaskEvent>(DOWNLOAD_EVENT, (message) => handler(message.payload));
}

// --- Phase 07: library & offline reader (ids only; pages are (chapter,index)) ---

export async function libraryList(args: {
  query?: string;
  sort?: string;
  filter?: string;
  limit: number;
  offset: number;
}): Promise<PagedResult<LibraryMangaDto>> {
  return invoke<PagedResult<LibraryMangaDto>>("library_list", args);
}

export async function libraryDownloadedChapters(mangaId: string): Promise<{
  chapters: DownloadedChapterDto[];
}> {
  return invoke<{ chapters: DownloadedChapterDto[] }>("library_get_downloaded_chapters", { mangaId });
}

export async function libraryStats(): Promise<LibraryStatsDto> {
  return invoke<LibraryStatsDto>("library_stats");
}

export async function libraryReconcile(): Promise<ReconcileReportDto> {
  return invoke<ReconcileReportDto>("library_reconcile");
}

export async function libraryRemoveChapter(chapterId: string): Promise<void> {
  await invoke<void>("library_remove_chapter", { chapterId });
}

export async function libraryRemoveMangaFiles(mangaId: string): Promise<RemoveMangaFilesDto> {
  return invoke<RemoveMangaFilesDto>("library_remove_manga_files", { mangaId });
}

/// Feature plan F3b — Rust opens the native picker (the renderer never sees a
/// path) and imports the chosen CBZ under `mangaId`. `null` = user cancelled.
/// The file name is generated server-side; only ids + metadata return.
export async function libraryImportCbz(args: {
  mangaId: string;
  chapterNumber?: number;
  chapterTitle?: string;
}): Promise<ImportResultDto | null> {
  return invoke<ImportResultDto | null>("library_import_cbz", args);
}

export async function readerOpenChapter(chapterId: string): Promise<ReaderManifestDto> {
  return invoke<ReaderManifestDto>("reader_open_chapter", { chapterId });
}

/// One page per call, binary (Rust `ipc::Response`). Normalizes to Uint8Array
/// regardless of the transport shape (ArrayBuffer | number[] | Uint8Array).
export async function readerGetPageBytes(
  chapterId: string,
  pageIndex: number,
): Promise<Uint8Array> {
  const response = await invoke<ArrayBuffer | Uint8Array | number[]>("reader_get_page_bytes", {
    chapterId,
    pageIndex,
  });
  if (response instanceof Uint8Array) {
    return response;
  }
  if (response instanceof ArrayBuffer) {
    return new Uint8Array(response);
  }
  return Uint8Array.from(response);
}

/// Approximate storage accounting and regenerable-cache cleanup (Phase 08).
/// No absolute path ever crosses this boundary.
export async function storageStatus(): Promise<StorageStatusDto> {
  return invoke<StorageStatusDto>("storage_status");
}

export async function clearRegenerableCache(): Promise<CacheCleanupDto> {
  return invoke<CacheCleanupDto>("clear_regenerable_cache");
}

/// Subscribe to tray "Open Downloads" clicks. Cleanup function MUST be
/// dropped on unmount (same rule as download events).
export async function subscribeTrayOpenDownloads(
  handler: () => void,
): Promise<UnlistenFn> {
  return listen<unknown>("tray-open-downloads", () => handler());
}

// --- Feature plan F1: new-chapter watch (ids + counts only) ---

export type WatchStateDto = {
  enabled: boolean | null;
  watchedAt: string | null;
  failCount: number;
};

export type ChapterAlertDto = {
  chapterId: string;
  title: string;
  detectedAt: string;
};

export type WatchTickDto = {
  checked: number;
  alertedChapters: number;
  failed: number;
};

export async function libraryGetWatch(mangaId: string): Promise<WatchStateDto> {
  return invoke<WatchStateDto>("library_get_watch", { mangaId });
}

export async function librarySetWatch(args: {
  mangaId: string;
  enabled: boolean | null;
}): Promise<void> {
  await invoke("library_set_watch", args);
}

export async function libraryListAlerts(mangaId: string): Promise<ChapterAlertDto[]> {
  return invoke<ChapterAlertDto[]>("library_list_alerts", { mangaId });
}

export async function libraryAckAlerts(mangaId: string): Promise<number> {
  return invoke<number>("library_ack_alerts", { mangaId });
}

export async function watchRunNow(): Promise<WatchTickDto> {
  return invoke<WatchTickDto>("watch_run_now");
}

export async function subscribeLibraryAlerts(
  handler: (payload: { newChapters: number }) => void,
): Promise<UnlistenFn> {
  return listen<{ newChapters: number }>("library-alerts", (message) =>
    handler(message.payload),
  );
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
  "SOURCE_FORBIDDEN",
  "SOURCE_RATE_LIMITED",
  "SOURCE_NETWORK",
  "SOURCE_PARSE_ERROR",
  "SOURCE_UPSTREAM",
  "LIBRARY_ITEM_NOT_FOUND",
  "LOCAL_FILE_MISSING",
  "LOCAL_FILE_CORRUPTED",
  "CBZ_INVALID",
  "UNSUPPORTED_IMAGE_FORMAT",
  "PAGE_RESOURCE_NOT_FOUND",
  "DELETE_FAILED",
  "STORAGE_OUTSIDE_LIBRARY_ROOT",
]);

function isAppErrorCode(value: unknown): value is AppError["code"] {
  return typeof value === "string" && APP_ERROR_CODES.has(value);
}
