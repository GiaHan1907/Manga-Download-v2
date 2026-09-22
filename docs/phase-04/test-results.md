# Phase 04 Test Results

**Môi trường:** Windows 11 x64 (10.0.26200), rustc/cargo 1.98.1 MSVC, Node v24, Git Bash. Mọi test DB chạy trên tempdir cô lập — không đụng DB thật của user (`%APPDATA%\com.manga1989.downloader\` chỉ được dev smoke sử dụng).

## Ma trận T04

| ID | Kịch bản | Bằng chứng | Kết quả |
|---|---|---|---|
| T04-01 | Fresh DB bootstrap + domain migrations | `phase04_domain_tables_exist_after_migrate`, `t04_03_schema_version_stable_across_reopen` (schemaVersion ≥ 2) | pass |
| T04-02 | Upgrade DB Phase 01 giữ `app_settings` | `t04_02_upgrade_preserves_existing_app_settings_row` | pass |
| T04-03 | Khởi động lần 2 không duplicate | `test_init_pool_idempotent` + count==1 sau reopen | pass |
| T04-04 | Duplicate `(source_id,source_manga_id)` → reuse id | `t04_04_duplicate_source_identity_reuses_manga` | pass |
| T04-05 | Duplicate chapter / FK vô hiệu | `t04_05_duplicate_chapter_identity_reuses_id_unknown_manga_rejected`; `fk_restrict_prevents_deleting_chapter_with_task_row` | pass |
| T04-06 | Favorite roundtrip + survive reconnect + reject unknown | `t04_06_favorite_roundtrip_and_reopen`, `unknown_manga_favorite_rejected` | pass |
| T04-07 | Progress sai cặp chapter/manga, index âm → reject, cũ còn nguyên | `t04_07_wrong_pairing_and_negative_index_rejected_existing_intact` | pass |
| T04-08 | Settings persist qua relaunch; bounds + unknown key reject | `t04_08_settings_survive_reopen_and_reject_invalid_patches`, `bounds_are_enforced`, `patch_rejects_unknown_keys_and_maps_camel_case` (deny_unknown_fields); frontend test "rejects out-of-bounds settings client-side" | pass |
| T04-09 | Library/task rỗng thật, không trộn demo | `t04_09_tasks_list_empty_and_status_filter_validated`; frontend `phase04.test.tsx` "distinguishes truthful empty from DB error"; test boundary "demo favorite … Favorites page is Local-only" | pass |
| T04-10 | DB lỗi → ErrorState recoverable, không thành empty | `phase04.test.tsx` retry recovery test (DB fail → alert → retry → empty thật) | pass |
| T04-11 | Phân trang bounded deterministic; page size sai reject | `t04_11_pagination_is_bounded_and_deterministic` (+ escape LIKE); `oversized_query_and_malformed_ids_rejected` | pass |
| T04-12 | DTO/Serde contract camelCase 2 phía | `serialized_keys_are_camel_case`, `wire_shape_is_camel_case_and_sanitized`, `error_codes_are_stable`; `src/types/backend.ts` mirror; `normalizeIpcError` contract test | pass |
| T04-13 | Regression Phase 01–03 | vitest 51/51 (toàn bộ phase03 matrix cập nhật còn chạy); cargo test 23/23; dev smoke dưới | pass (smoke chờ mục §2) |
| T04-14 | Audit bảo mật | `git diff` capabilities/tauri.conf: **không đổi** từ Phase 02 (chỉ thêm `withGlobalTauri:false` ở review P01); app commands không cần permission; grep không fetch/SQL raw/path command; DB path chỉ từ app_data API | pass |
| T04-15 | Gates + Windows smoke | bảng命令 dưới | pass / smoke pending |

## Lệnh đã chạy (thật)

| Lệnh | Exit | Ghi chú |
|---|---|---|
| `cargo fmt --all -- --check` | 0 (fmt đã apply trước đó) | |
| `cargo clippy --all-targets -- -D warnings` | 0 | sạch hoàn toàn |
| `cargo test` | 0 | 23 passed / 0 failed (tempdir isolate) |
| `cargo check` | 0 | |
| `npm run lint` | 0 | |
| `npm run typecheck` | 0 | |
| `npm run test` | 0 | 51 passed / 5 files |
| `npm run build` | 0 | 352.91 kB JS, gzip 109.27 kB |
| `npm run tauri dev` smoke | xem §2 | |
| `npm run tauri build` | chưa chạy lại | đã pass ở review Phase 02; Rust đã đổi nhiều → **gate bắt buộc trước release**, đề xuất chạy ngay khi user cho phép |

## 2. Windows dev smoke (Phase 04) — PASS

`npm run tauri dev` (2026-09-22): rebuild tự động theo code mới, exe chạy, đóng cửa sổ exit code **0**, log không panic/error. Bằng chứng trên DB THẬT của user (nâng cấp từ Phase 01, không reset):

- `_sqlx_migrations` chứa bản ghi migration `0002_domain.sql` → upgrade path live hoạt động (T04-02 thực địa).
- 7 bảng domain (`sources, mangas, chapters, download_tasks, download_pages, reading_progress, favorites`) đều hiện diện trong file DB.
- `storage_service::prepare` tạo thật `covers/` và `cache/` trong app-data dir.

Còn chờ mắt người (manual, như các phase trước): thao tác Settings save → restart app → giá trị còn nguyên trên UI; phân biệt empty vs error khi DB khóa; các mức display scaling.
