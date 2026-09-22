# IPC Contract — React ↔ Rust (Phase 04 + Phase 05)

Mọi lời gọi phía frontend đi qua `src/shared/api/desktop.ts` (duy nhất). Tham số và payload serialized **camelCase**; command Rust khai báo snake_case, Tauri tự map. Lỗi trả về luôn là envelope đã kiểm chứng, không bao giờ raw SQL/path/stack.

## Error envelope

```json
{ "code": "DATABASE_INIT_FAILED | VALIDATION_ERROR | NOT_FOUND | DB_CONFLICT | UNSUPPORTED_IN_THIS_PHASE | INTERNAL_ERROR
        | INVALID_URL | UNSUPPORTED_SOURCE | SOURCE_DISABLED | UNSUPPORTED_OPERATION | SOURCE_NOT_FOUND | SOURCE_RATE_LIMITED | SOURCE_NETWORK | SOURCE_PARSE_ERROR | SOURCE_UPSTREAM",
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
| `app_save_reading_progress` | `{ mangaId, chapterId, pageIndex }` | `ReadingProgressDto` | validate ownership chapter∈manga trong transaction; `0 ≤ pageIndex ≤ 10000`; write lỗi không phá progress cũ |
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

## DTO định nghĩa tại `src/types/backend.ts`

`LocalMangaDto` (kèm `chapterCount`, `favorite`), `LocalChapterDto`, `LocalMangaDetailDto`, `ReadingProgressDto`, `DownloadTaskDto`, `AppSettingsDto`, `PagedResult<T>`.

## DTO Phase 05 tại `src/types/source.ts`

`SourceViewDto` (flatten `SourceInfo` + `enabled/builtin/baseUrl`), `RemoteMangaSummaryDto`, `RemoteChapterDto`, `RemoteMangaDetailDto` (flatten summary + `chapters`, `savedMangaId`, `availableOffline`), `SearchResultDto` (`items`, `sourceErrors[]`, `hasMore`), `SaveResultDto`.

## Mô hình tiêu thụ phía UI

- `localServices.library` (`src/services/local-library.api.ts`) — màn **Local**: Library, Favorites.
- `localServices.source` (`src/services/source.api.ts`) — **Production discovery** (Phase 05): Discover live tabs, `/remote/...`, Settings → Sources.
- `services` (`provider.ts`) — sandbox **Demo**: Manga Detail/Downloads/Reader/Home + tab Demo sandbox (mock fixtures, nhãn Demo).
- `services.settings` — đã là adapter THẬT (`settings.api.ts`) từ Phase 04.
- Quy tắc: DB error ≠ empty library; provider error ≠ no results. UI phải phân biệt `ErrorState` (retry) / danh sách `sourceErrors` / `EmptyState`.
