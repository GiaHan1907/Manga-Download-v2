# Phase 04 — Database & Backend Core: Implementation Notes

**Ngày:** 2026-09-22 · **Implementer:** Qoder agent · **Review:** tự review theo brief (workflow hiện hành, xem §10)

## 1. P04-01 — Audit repo (trạng thái thật trước khi code)

- Git: `master` sạch, đồng bộ `origin/master` tại `5888d5a`; Phase 03 đã ACCEPT 2 vòng review.
- Migration duy nhất: `src-tauri/migrations/0001_init.sql` (`app_settings` key/value_json/updated_at, TEXT ISO-8601 UTC) → **migration mới = `0002_domain.sql`**.
- Rust: `AppState { db: SqlitePool }` (1 pool, max 5 conn, foreign_keys ON, create_if_missing, không WAL — giữ nguyên quyết định Phase 01 §7); `sqlx::migrate!("./migrations")` nhúng; 2 command `app_get_info`/`app_health_check`; `AppError` serialize `{code,message,retryable}`; **chỉ runtime query, không dùng macro `query!`** (không DATABASE_URL) — giữ convention này.
- Deps Rust hiện có: tauri 2, serde, sqlx 0.8 (sqlite, runtime-tokio, migrate, macros), thiserror 1. Phase 04 thêm: `uuid` (v4, ID nội bộ), `chrono` (timestamp UTC ISO-8601 theo convention Phase 01). Không gì khác.
- Frontend: IPC boundary `src/shared/api/desktop.ts` (sole `invoke`), service boundary `src/services/provider.ts` (5 interface mock), hooks `useAsyncData`/`useServicesVersion`. **React Query/Zustand không tồn tại** — brief §2.2 minh hoạ; dùng hạ tầng có sẵn (existing conventions take precedence), ghi là deviation.
- Capability: `core:default` + 5 window permission; **app-defined command KHÔNG cần permission mới** (Tauri 2) → capabilities giữ nguyên, chỉ đăng ký command trong `generate_handler!`.
- `docs/ARCHITECTURE.md`, `docs/IPC_CONTRACT.md` brief yêu cầu đọc: chưa tồn tại → **tạo mới trong P04-07** như tài liệu authoritative (deviation đã ghi).
- `AGENTS.md`: brief gợi ý cập nhật; **bỏ qua** — file thuộc user, bất biến (rule HANDOFF §3.7).

### Map Phase 03 mock → Phase 04 (§2.8 boundary)

| Service interface | Quyết định Phase 04 | Lý do |
|---|---|---|
| `SettingsService` | **REAL** (SQLite, key `settings.v1`) | Settings là local đã có backend |
| `LibraryService` (màn Library/Favorites) | **REAL Local** — service interface mới `LocalLibraryService`; DB trống tới khi Phase 05 đăng ký manga | Không trộn demo fixture vào production view |
| `CatalogService` | **GIỮ DEMO** (fixtures, badge) | Nguồn web = Phase 05 |
| `DownloadService` | **GIỮ DEMO** (transitions mock, nhãn simulated) | Worker thật = Phase 06; schema tasks chỉ khai báo |
| `ReaderService` | pages: **GIỮ DEMO**; progress: lệnh thật `get/save_reading_progress` sẵn sàng cho manga đăng ký (UI reader demo vẫn dùng mock) | File thật = Phase 07 |
| `windowService` / `desktop.ts` | giữ | — |

Library/Favorites UI: cùng layout Phase 03, nguồn dữ liệu đổi sang IPC thật; phân biệt rõ **empty** ("chưa đăng ký manga local nào — danh sách sẽ có từ Phase 05") vs **DB error** (ErrorState + Retry) theo T04-09/10. Màn giữ demo giữ nguyên + badge Demo.

### Health & hợp đồng lỗi
- `app_health_check` mở rộng thêm `schemaVersion` (MAX(version) từ `_sqlx_migrations`) — additive, không phá test cũ.
- `AppError` code mở rộng: `VALIDATION_ERROR`, `NOT_FOUND`, `DB_NOT_READY`, `DB_CONFLICT`, `UNSUPPORTED_IN_THIS_PHASE` (giữ 2 code cũ).

## 2. P04-02…06 — kết quả

- `0002_domain.sql`: 7 bảng + 7 index, CHECK/UNIQUE/FK-RESTRICT (xem `schema.md`).
- Rust layers: `domain/` (settings, DTO, validators), `db/repositories/` (settings/manga/favorite/progress/task), `services/` (settings/catalog/favorites/reading/storage), 10 command mới + health thêm `schemaVersion`. fmt + clippy `-D warnings` sạch; **cargo test 23/23** (T04-01→12 đều có test tương ứng).
- Frontend: `desktop.ts` thêm 12 hàm typed; `types/backend.ts` mirror DTO; `local-library.service/.api` + `settings.api` (adapter thật); `provider.ts` tách `services` (demo) và `localServices` (production). Library/Favorites/Settings = Local thật, phân biệt empty ≠ DB error + Retry; Discover/Detail/Downloads/Reader/Home giữ sandbox demo có nhãn. **vitest 51/51**, lint/typecheck/build pass.
- Deviations (có giải trình): không thêm React Query/Zustand; tạo mới `docs/ARCHITECTURE.md` + `docs/IPC_CONTRACT.md`.

## 3→9 — xem `test-results.md` (ma trận lệnh) và `self-review.md` (findings)

## 3-9. Kết quả / ma trận / handoff — bổ sung ở P04-07
