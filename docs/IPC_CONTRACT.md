# IPC Contract — React ↔ Rust (Phase 04 + 05 + 06 + 07 + 08)

Mọi lời gọi phía frontend đi qua `src/shared/api/desktop.ts` (duy nhất). Tham số và payload serialized **camelCase**; command Rust khai báo snake_case, Tauri tự map. Lỗi trả về luôn là envelope đã kiểm chứng, không bao giờ raw SQL/path/stack.

## Error envelope

```json
{ "code": "DATABASE_INIT_FAILED | VALIDATION_ERROR | NOT_FOUND | DB_CONFLICT | UNSUPPORTED_IN_THIS_PHASE | INTERNAL_ERROR
        | INVALID_URL | UNSUPPORTED_SOURCE | SOURCE_DISABLED | UNSUPPORTED_OPERATION | SOURCE_NOT_FOUND | SOURCE_FORBIDDEN | SOURCE_RATE_LIMITED | SOURCE_NETWORK | SOURCE_PARSE_ERROR | SOURCE_UPSTREAM
        | LIBRARY_ITEM_NOT_FOUND | LOCAL_FILE_MISSING | LOCAL_FILE_CORRUPTED | CBZ_INVALID | UNSUPPORTED_IMAGE_FORMAT | PAGE_RESOURCE_NOT_FOUND | DELETE_FAILED | STORAGE_OUTSIDE_LIBRARY_ROOT",
  "message": "string — an toàn cho UI",
  "retryable": true }
```

Frontend parse qua `normalizeIpcError(err): AppError` (`contracts.ts`). Code lạ → `INTERNAL_ERROR`. `retryable=true`: lỗi DB init/migration và nhóm source Network/RateLimited/Upstream.

## Commands

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `app_get_info` | — | `{ name, version, platform }` | Phase 01 |
| `app_health_check` | — | `{ status, database, schemaVersion }` | Phase 01 + `schemaVersion` (MAX version `_sqlx_migrations`) Phase 04 |
| `app_get_settings` | — | `AppSettingsDto` | SQLite key `settings.v1`; giá trị hỏng → default |
| `app_update_settings` | `{ input: SettingsPatchDto }` | `AppSettingsDto` đã merge+validate | `deny_unknown_fields`; bounds 1–5 / 1–8; enum khóa; lỗi → `VALIDATION_ERROR` |
| `app_reset_settings` | — | `AppSettingsDto` mặc định | |
| `app_list_local_mangas` | `{ query?, limit?, offset? }` | `PagedResult<LocalMangaDto>` | limit 1..=100 (default 20), offset ≤100k; sort title NOCASE + id; query được escape LIKE |
| `app_get_local_manga` | `{ mangaId }` | `LocalMangaDetailDto` | id sai định dạng → `VALIDATION_ERROR`; không tồn tại → `NOT_FOUND` |
| `app_set_manga_favorite` | `{ mangaId, favorite }` | `boolean` | chỉ manga đã đăng ký; idempotent |
| `app_list_favorite_mangas` | `{ limit?, offset? }` | `PagedResult<LocalMangaDto>` | newest first |
| `app_get_reading_progress` | `{ mangaId }` | `ReadingProgressDto \| null` | |
| `app_save_reading_progress` | `{ mangaId, chapterId, pageIndex, scrollOffset?, pageTotal? }` | `ReadingProgressDto` (+`scrollOffset,pageTotal`) | validate ownership chapter∈manga trong MỘT transaction; `0 ≤ pageIndex ≤ 10000`; `scrollOffset∈0..=1`; stamp `mangas.last_read_at` (Phase 07); write lỗi không phá progress cũ |
| `app_list_local_download_tasks` | `{ status?, limit?, offset? }` | `PagedResult<DownloadTaskDto>` | CHỈ ĐỌC (Phase 06 mới ghi); status sai → `VALIDATION_ERROR` |

**Không tồn tại** command nào nhận đường dẫn file tuyệt đối, SQL, URL tùy ý để fetch, headers từ renderer hay generic insert metadata. `save_discovered_manga` chỉ nhận cặp ID và tự re-fetch từ provider.

## Commands Phase 05 — source engine & discovery

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `list_supported_sources` | — | `SourceViewDto[]` | Registry động: MangaDex built-in + hàng `sources` adapter `madara` hợp lệ; merge `enabled` từ DB |
| `add_manga_source` | `{ displayName, baseUrl }` | `SourceViewDto` | baseUrl phải HTTPS, host public, không port lạ/credential; id do host `madara-<host>`; registry rebuild sau ghi |
| `remove_manga_source` | `{ sourceId }` | — | builtin (`mangadex`) → `UNSUPPORTED_IN_THIS_PHASE`; registry rebuild |
| `set_source_enabled` | `{ sourceId, enabled }` | `boolean` | chỉ id có trong registry; tạo row cho builtin khi toggle lần đầu |
| `search_manga` | `{ query, sourceIds?, page? }` | `SearchResultDto` | query 1..=200 ký tự, page 1..=50, ≤10 sourceIds; lỗi từng nguồn cô lập trong `sourceErrors` |
| `resolve_manga_url` | `{ url }` | `RemoteMangaDetailDto` | chỉ URL thuộc adapter đã đăng ký (exact host); URL lạ không bị fetch; `INVALID_URL`/`UNSUPPORTED_SOURCE`/`SOURCE_DISABLED` |
| `get_remote_manga_details` | `{ sourceId, sourceMangaId }` | `RemoteMangaDetailDto` | kèm `savedMangaId` nếu đã có trong catalog; `availableOffline` luôn false ở Phase 05 |
| `get_remote_chapters` | `{ sourceId, sourceMangaId }` | `RemoteChapterDto[]` | cùng nguồn dữ liệu với detail; order ổn định |
| `save_discovered_manga` | `{ sourceId, sourceMangaId }` | `{ mangaId, savedChapterCount, updated }` | re-fetch trusted + MỘT transaction upsert sources→mangas→chapters; không xóa, không đụng favorites/progress |

## Commands Phase 06 — download engine (chapter/task id-only)

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `queue_chapter_downloads` | `{ chapterIds[] (1..100), format?: "cbz"\|"folder", priority?: -10..10 }` | `QueueResultDto { tasks, reused, errors[], queuedAt }` | KHÔNG nhận URL/headers/path. Format mặc định từ Settings (pdf bị từ chối). Dedup: chapter đã có task non-terminal → `reused`. Lỗi từng mục (`NOT_FOUND`/`UNSUPPORTED_OPERATION` khi nguồn thiếu capability `pages`) không fail cả batch. Một transaction |
| `get_download_task` | `{ taskId }` | `DownloadTaskDto` | snapshot bền vững; `NOT_FOUND` |
| `pause_download` | `{ taskId }` | `DownloadTaskDto` | queued→paused ngay; đang chạy → flag hợp tác, worker tự chốt `pausing→paused` khi writes dừng; transition sai → `VALIDATION_ERROR` |
| `resume_download` | `{ taskId }` | `DownloadTaskDto` | chỉ paused→queued |
| `cancel_download` | `{ taskId }` | `DownloadTaskDto` | queued/paused ngay; worker nhận flag tự chốt cancelled; staging của task bị xóa, **không đụng file đã publish** |
| `retry_download` | `{ taskId }` | `DownloadTaskDto` | chỉ failed→queued; giữ attempts/backoff đã lưu |
| `set_download_priority` | `{ taskId, priority }` | `DownloadTaskDto` | chỉ queued/paused/failed |
| `app_list_local_download_tasks` (mở rộng) | `{ status?, limit?, offset? }` | `PagedResult<DownloadTaskDto>` | status nay nhận cả 10 lifecycle states; DTO thêm `attempts/nextRetryAt/errorMessage/revision/finishedAt/outputRelpath/chapterTitle/mangaTitle` |

**Event** `download-task-updated` → payload `DownloadTaskEvent { task, downloadedBytes, speedBytesPerSecond|null, occurredAt }`: progress gộp ≥250ms/task, chuyển trạng thái tức thì; `task.revision` tăng đơn điệu để UI chặn frame cũ ghi đè frame mới. Frontend subscribe qua `desktop.subscribeDownloadTask()` và **bắt buộc unlisten khi teardown**. `DownloadTaskDto` không bao giờ chứa URL trang/headers/đường dẫn tuyệt đối; `outputRelpath` là đường dẫn tương đối dưới app library root, chỉ hiển thị, không phải input.

## Commands Phase 07 — library & offline reader (id-only, binary per page)

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `library_list` | `{ query?, sort?('lastRead'│'recentDownload'│'title'│'chapters'), filter?('all'│'downloaded'│'partial'│'favorite'), limit(1..100), offset }` | `PagedResult<LibraryMangaDto>` | aggregate 1 query (counts, size, progress, hasMissing) — không N+1; query escape LIKE |
| `library_get_downloaded_chapters` | `{ mangaId }` | `{ chapters: DownloadedChapterDto[] }` | `artifactOk` = Rust fs-check tại lúc gọi |
| `library_reconcile` | — | `{ checked, healthy, missing, corrupted }` | phi phá hủy: missing⇒clear cờ offline, corrupted⇒clear cờ + GIỮ file; không bao giờ xóa |
| `library_remove_chapter` | `{ chapterId }` | — | path lấy từ DB, validate relpath + under library root; **xóa disk trước, DB sau**; fail ⇒ `DELETE_FAILED`, trạng thái nguyên |
| `library_remove_manga_files` | `{ mangaId }` | `{ removed[], failed[] }` | per-item trung thực; giữ metadata/favorites/progress |
| `reader_open_chapter` | `{ chapterId }` | `ReaderManifestDto` | chỉ chapter có `local_path`; `LOCAL_FILE_MISSING`/`CBZ_INVALID`/`LOCAL_FILE_CORRUPTED`; manifest KHÔNG chứa path/tên file; kèm prev/Next chương đã tải + startPage resume |
| `reader_get_page_bytes` | `{ chapterId, pageIndex }` | **binary** (`ipc::Response` → JS `ArrayBuffer`) | 1 trang/lần, cap 20MB; entry archive là basename-only; magic-check; `PAGE_RESOURCE_NOT_FOUND` |

`app_list_local_download_tasks`… (Phase 06 giữ nguyên). Reader page: UI dùng `URL.createObjectURL` blob có thu hồi; CSP `img-src` thêm `blob:` (duyệt trong Phase 07); capabilities không đổi.

## Commands Phase 08 — integration, storage & Windows (id-only, approximate sizes)

| Command | Input | Output | Ghi chú |
|---|---|---|---|
| `library_stats` | — | `LibraryStatsDto { savedMangas, downloadedChapters, localSizeBytes, favorites }` | 1 SQL aggregate cho Home thật; không N+1 |
| `storage_status` | — | `StorageStatusDto { libraryBytes, cacheBytes, stagingDirs, activeTasks, rootAvailable, approximate:true }` | kích thước từ DB + walk root app-owned; symlink bỏ qua; **không absolute path** |
| `clear_regenerable_cache` | — | `CacheCleanupDto { removedDirs, freedBytes, keptActiveDirs }` | chỉ staging của task terminal/unknown + covers; active-set chặn tuyệt đối; idempotent; cũng chạy tự động ở startup sau recovery |

**Settings keys mới** (`AppSettingsDto`): `notificationsEnabled` (bool, default true) — Rust notify theo chapter-terminal; `closeToTray` (bool, default false) — X ẩn cửa sổ. Validate trong `domain/settings.rs`; `RuntimeFlags` mirror cho sync path; command save/reset đồng bộ flag sau persist. UI bỏ option `pdf` (engine vẫn từ chối pdf tại queue như cũ).

**Event** `tray-open-downloads` → payload rỗng: tín hiệu điều hướng từ tray menu; subscribe qua `desktop.subscribeTrayOpenDownloads()`, unlisten khi teardown (bridge duy nhất cấp app).

**Invalidation frontend:** `src/shared/hooks/useInvalidation.ts` — 7 domain transient (`home/library/favorites/downloads/progress/settings/sources`); event/status mutation CHỈ bump domain, screens refetch snapshot từ Rust. Không phải kênh dữ liệu mới — không thêm IPC.

Capabilities KHÔNG đổi (notification gọi từ Rust, không qua IPC permission).

## DTO định nghĩa tại `src/types/backend.ts`

`LocalMangaDto` (kèm `chapterCount`, `favorite`), `LocalChapterDto` (+`downloaded`, `outputFormat` — badge offline do Rust kiểm chứng), `LocalMangaDetailDto`, `ReadingProgressDto`, `DownloadTaskDto`, `AppSettingsDto`, `PagedResult<T>`. Phase 06 thêm `src/types/downloads.ts` (`QueueResultDto`, `DownloadTaskEvent`, bảng transition UI, format helpers) và `localServices.downloads` (`src/services/downloads.api.ts`).

## DTO Phase 05 tại `src/types/source.ts`

`SourceViewDto` (flatten `SourceInfo` + `enabled/builtin/baseUrl`), `RemoteMangaSummaryDto`, `RemoteChapterDto`, `RemoteMangaDetailDto` (flatten summary + `chapters`, `savedMangaId`, `availableOffline`), `SearchResultDto` (`items`, `sourceErrors[]`, `hasMore`), `SaveResultDto`.

## Mô hình tiêu thụ phía UI

- `localServices.library` (`src/services/local-library.api.ts`) — màn **Local**: Library, Favorites.
- `localServices.source` (`src/services/source.api.ts`) — **Production discovery** (Phase 05): Discover live tabs, `/remote/...`, Settings → Sources.
- `localServices.downloads` (`src/services/downloads.api.ts`) — **Production queue** (Phase 06): Downloads manager, batch submit, event `download-task-updated`.
- `localServices.libraryV2` / `localServices.reader` (`library.api.ts` / `reader.api.ts`) — **Production library + offline reader** (Phase 07): Library/LocalManga/`/read/:chapterId`.
- `services` (`provider.ts`) — sandbox **Demo**: Manga Detail/Downloads/Reader/Home + tab Demo sandbox (mock fixtures, nhãn Demo).
- `services.settings` — đã là adapter THẬT (`settings.api.ts`) từ Phase 04.
- Quy tắc: DB error ≠ empty library; provider error ≠ no results. UI phải phân biệt `ErrorState` (retry) / danh sách `sourceErrors` / `EmptyState`.
