# IPC Contract — React ↔ Rust (Phase 04)

Mọi lời gọi phía frontend đi qua `src/shared/api/desktop.ts` (duy nhất). Tham số và payload serialized **camelCase**; command Rust khai báo snake_case, Tauri tự map. Lỗi trả về luôn là envelope đã kiểm chứng, không bao giờ raw SQL/path/stack.

## Error envelope

```json
{ "code": "DATABASE_INIT_FAILED | VALIDATION_ERROR | NOT_FOUND | DB_CONFLICT | UNSUPPORTED_IN_THIS_PHASE | INTERNAL_ERROR",
  "message": "string — an toàn cho UI",
  "retryable": true }
```

Frontend parse qua `normalizeIpcError(err): AppError` (`contracts.ts`). Code lạ → `INTERNAL_ERROR`. `retryable=true` duy nhất với lỗi DB init/migration.

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

**Không tồn tại** command nào nhận đường dẫn file tuyệt đối, SQL, URL tùy ý hay ghi metadata manga từ renderer (upsert là API repository nội bộ cho Phase 05).

## DTO định nghĩa tại `src/types/backend.ts`

`LocalMangaDto` (kèm `chapterCount`, `favorite`), `LocalChapterDto`, `LocalMangaDetailDto`, `ReadingProgressDto`, `DownloadTaskDto`, `AppSettingsDto`, `PagedResult<T>`.

## Mô hình tiêu thụ phía UI

- `localServices.library` (`src/services/local-library.api.ts`) — màn **Local**: Library, Favorites.
- `services` (`provider.ts`) — sandbox **Demo**: Discover/Manga Detail/Downloads/Reader/Home (mock fixtures, nhãn Demo).
- `services.settings` — đã là adapter THẬT (`settings.api.ts`) từ Phase 04.
- Quy tắc: DB error ≠ empty library. UI phải phân biệt qua `ErrorState` (retry) vs `EmptyState`.
