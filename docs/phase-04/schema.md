# Phase 04 Schema — `0002_domain.sql`

Giữ nguyên `0001_init.sql` (`app_settings`). Convention: PK/FK là TEXT UUID v4 do Rust sinh; timestamp TEXT UTC ISO-8601 (millis); `PRAGMA foreign_keys=ON` mỗi connection; busy_timeout 5s; không WAL (quyết định Phase 01); FK `ON DELETE RESTRICT` (chỉ `download_pages` cascade theo task của nó).

## Bảng

| Bảng | Khóa & ràng buộc | Chỉ mục | Phase 04 hành vi |
|---|---|---|---|
| `sources` | PK `id`; `enabled` CHECK(0/1) | — | Identity đăng ký; service-only |
| `mangas` | PK `id`; FK `source_id` RESTRICT; `UNIQUE(source_id, source_manga_id)`; `status` CHECK(ongoing/completed/unknown); title NOT NULL | `idx_mangas_updated_at`, `idx_mangas_title(NOCASE)` | Metadata thật qua repository upsert idempotent; **không** seed demo |
| `chapters` | PK `id`; FK `manga_id` RESTRICT; `UNIQUE(manga_id, source_chapter_id)`; `page_count >= 0` | `idx_chapters_manga` | Sắp xếp deterministic: number NULL-last, number, created_at, id |
| `download_tasks` | PK `id`; FK `chapter_id` RESTRICT; `status` CHECK 5 giá trị; counters `>= 0` | `idx_tasks_status_created`, `idx_tasks_chapter` | **Schema only** — Phase 04 không tạo task, chỉ command đọc `app_list_local_download_tasks` |
| `download_pages` | PK `(task_id, page_index)`; FK `task_id` CASCADE; `page_index >= 0` | — | Checkpoint cho Phase 06; không ghi trong phase này |
| `reading_progress` | PK `manga_id` (1 dòng/manga); FK `manga_id` + `chapter_id` RESTRICT; `page_index >= 0` | `idx_progress_updated` | Quyền sở hữu chapter→manga kiểm tra trong transaction service (FK đơn không đủ) |
| `favorites` | PK `manga_id`; FK RESTRICT | `idx_favorites_created` | Set/unset idempotent; reject manga chưa đăng ký |

## Upgrade / reopen (T04-01→03)

- Fresh: 0001+0002 apply, `schemaVersion = 2` (MAX `_sqlx_migrations.version`).
- Upgrade DB Phase 01 có dòng `app_settings`: migration chạy chồng, dòng cũ **không đổi** — test `t04_02_upgrade_preserves_existing_app_settings_row`.
- Reopen: SQLx history table chống apply lại; test `t04_03_schema_version_stable_across_reopen` + `test_init_pool_idempotent`.
- Rollback: không tự động — forward-only; hoàn tác = migration mới ngược chiều, không bao giờ reset DB user.

## Kiểm ràng buộc đã chứng minh bằng test

UNIQUE source identity (reuse id), UNIQUE chapter identity, FK RESTRICT chặn xóa manga còn task, negative page_index chặn bởi CHECK+validation, cross-manga pairing chặn trong tx.
