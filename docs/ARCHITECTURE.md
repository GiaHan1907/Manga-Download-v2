# Architecture

## Tổng quan

Desktop app Windows local-first: **React 19 + TypeScript + Vite** trong WebView Tauri 2; **Rust** sở hữu toàn bộ I/O (SQLite qua SQLx, window API); frontend không chạm filesystem/network/DB trực tiếp.

```
React screens (src/features, src/components)
  ├─ Demo sandbox ── provider.services ── mock fixtures (src/services/mock)
  └─ Local production ─ provider.localServices / settings.api
                     └─ shared/api/desktop.ts (sole invoke boundary)
                           ├─ Rust commands (src-tauri/src/commands)
                           │     ├─ services (business invariants, tx)
                           │     │     └─ db/repositories (parameterized SQLx runtime queries)
                           │     │           └─ SQLite @ app_data_dir (migrations 0001→0002→0003…)
                           │     └─ [P05] discovery_service ─ SourceRegistry (Arc<Mutex>)
                           │                                   └─ SourceAdapter impls (mangadex, madara generic)
                           │                                         └─ GuardedHttp (Rust owns ALL provider HTTP)
```

## Tầng Rust (Phase 04)

- `commands/`: giải mã + validate input (bounded ids/strings, page 1..=100), không chứa SQL nghiệp vụ. Đăng ký tường minh trong `generate_handler!`.
- `services/`: invariant kinh doanh — validate settings, ownership chapter→manga trong transaction, phân biệt not-found, chuẩn bị storage root (`covers/`, `cache/`) nhưng **không** mở đường dẫn tùy ý.
- `db/repositories/`: SQLx runtime query (không macro `query!` — tránh DATABASE_URL build-time), upsert idempotent theo `(source_id, source_manga_id)` / `(manga_id, source_chapter_id)`, pagination có giới hạn + escape LIKE.
- `domain/`: typed DTO + error-safe helpers; `error.rs`: AppError → wire `{code, message, retryable}` camelCase, message tĩnh đã sanitize.
- `AppState { db: SqlitePool }` quản lý một lần trong `setup` (block_on init → fail sớm, health không bao giờ giả `ready`).
- `db/mod.rs`: một pool duy nhất, foreign_keys ON, busy_timeout 5s, **không WAL** (quyết định Phase 01), migrations nhúng, forward-only.

## Tầng Rust Phase 05 — source engine

- `sources/adapter.rs`: trait `SourceAdapter` (async) + `SourceError` taxonomy (INVALID_URL, UNSUPPORTED_SOURCE, SOURCE_DISABLED, SOURCE_NOT_FOUND, SOURCE_RATE_LIMITED, SOURCE_NETWORK, SOURCE_PARSE_ERROR, SOURCE_UPSTREAM) — map vào AppError envelope.
- `sources/registry.rs`: `SourceRegistry::from_rows` = built-in MangaDex + các hàng `sources` có `adapter='madara'` hợp lệ (hàng hỏng bị skip kèm log, không fatal). Giữ trong `AppState.registry: Arc<Mutex<SourceRegistry>>`; command clone snapshot ra khỏi lock trước khi await.
- `sources/http.rs`: `GuardedHttp` — HTTPS-only, exact-host allowlist theo adapter, chặn port lạ/credential/IP literal-private, resolve DNS rồi kiểm mọi địa chỉ public (loopback/private/CGNAT/ULA/link-local/reserved fail), redirect follow thủ công ≤3 hop với re-validate từng hop, cap body 2 MB, timeout 15s, pace ≥3s giữa request, UA định danh app. `Transport` trait để test bằng `MockTransport`.
- `sources/providers/`: `mangadex.rs` (API chính thức, đọc metadata; live smoke BLOCKED vì mạng reset TLS — ghi trong source-decision); `madara_generic.rs` (site WordPress/Madara người dùng đăng ký: search qua admin-ajax có sẵn của site, detail parse HTML public, chapter id = slug); `fixture.rs` (test-only, deterministic, không bao giờ vào registry production).
- `services/discovery_service.rs`: fan-out search cô lập lỗi theo nguồn + dedupe theo (sourceId, sourceMangaId); resolve-then-request; `save` re-fetch từ provider (không tin metadata từ UI) rồi MỘT transaction upsert sources→mangas→chapters, không xóa chapter, không đụng favorites/progress; registry rebuild sau add/remove.
- `commands/discovery.rs`: 9 command hẹp (§ IPC_CONTRACT). Bounds: query ≤200, page ≤50, ≤10 sourceIds, URL ≤2048; mọi sourceId được re-validate phía Rust.

## Tầng frontend

- Service interfaces (`src/services/*.service.ts`) là boundary duy nhất giữa màn hình và dữ liệu. `provider.ts` quyết định mock hay IPC-backed; Phase 05 bổ sung `localServices.source` (`source.api.ts`) cho discovery thật — screens không tự invoke.
- Discover (Phase 05): tab mặc định là **Tìm kiếm (thật)** + **Import URL (thật)** (Rust IPC); sandbox **Demo** tách riêng có nhãn; route mới `/remote/:sourceId/:sourceMangaId` = remote preview, save là hành động tường minh.
- Demo vs Local/Production tách biệt tường thuật (badges, empty-state copy, "Saved ≠ downloaded", "Chưa tải — Phase 06").
- Routing HashRouter + `routes.ts` contract; state ephemeral của demo nằm trong mock store (`useSyncExternalStore` qua `useServicesVersion`).

## Bảo mật

- Capability: `core:default` + 5 window permission cụ thể, window `main` duy nhất. App-defined commands không cần permission.
- CSP: `script-src 'self'`, style `'unsafe-inline'` (React inline styles), không wildcard, không remote.
- Không shell/fs/http plugin, không generic fetch/SQL/path command. DB path chỉ từ `app.path().app_data_dir()`.
- Phase 05: HTTP provider **chỉ trong Rust**, luôn qua `GuardedHttp` theo policy exact-host của từng adapter; renderer chỉ gửi query/ID/URL-để-đối-khớp, không gửi URL tùy ý để fetch. Hai ngoại lệ người-dùng-nhập được khai báo tường minh (review 4 vòng): `resolve_manga_url` (URL import chính chủ — parse + khớp exact-host TRƯỚC khi fetch) và `add_manga_source` (base_url khi đăng ký Madara source — HTTPS, host đơn, không IP literal/port lạ; mọi request sau đó vẫn qua GuardedHttp). Source do người dùng đăng ký bị giới hạn cùng chính sách (pace ≥3s). SSRF/rebinding chặn ở cả literal và IP đã resolve.
- Không log/IPC nào lộ SQL, đường dẫn tuyệt đối của user, stack trace.

## Migration policy

`app_settings` Phase 01 giữ nguyên; schema Phase 04 = `0002_domain.sql` (xem `docs/phase-04/schema.md`); Phase 05 thêm `0003_source_config.sql` (cột `sources.adapter` default `mangadex` + `sources.base_url`, forward-only, verified trên DB thật lên schema 3). Phase tiếp theo thêm số thứ tự mới, không sửa migration đã phát hành. **Ngoại lệ duy nhất, có kiểm soát:** SQL của `0004_download_engine.sql` được thay tại runtime bằng bản repaired (`src/db/migrations_repaired/`, nạp qua `db::production_migrator()`) — bản gốc chứa `PRAGMA foreign_keys = OFF` vốn bị SQLite BỎ QUA bên trong transaction của sqlx, khiến `DROP TABLE download_tasks` CASCADE-wipe `download_pages` khi upgrade 2→4; bản repaired rebuild bảng con trước rồi swap, giữ NGUYÊN checksum gốc để DB đã apply không lệch validate (test thuận + negative-control trong `src/db/mod.rs::t10_*`).
