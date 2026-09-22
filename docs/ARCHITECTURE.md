# Architecture

## Tổng quan

Desktop app Windows local-first: **React 19 + TypeScript + Vite** trong WebView Tauri 2; **Rust** sở hữu toàn bộ I/O (SQLite qua SQLx, window API); frontend không chạm filesystem/network/DB trực tiếp.

```
React screens (src/features, src/components)
  ├─ Demo sandbox ── provider.services ── mock fixtures (src/services/mock)
  └─ Local production ─ provider.localServices / settings.api
                     └─ shared/api/desktop.ts (sole invoke boundary)
                            └─ Rust commands (src-tauri/src/commands)
                                  └─ services (business invariants, tx)
                                        └─ db/repositories (parameterized SQLx runtime queries)
                                              └─ SQLite @ app_data_dir (migrations 0001→0002…)
```

## Tầng Rust (Phase 04)

- `commands/`: giải mã + validate input (bounded ids/strings, page 1..=100), không chứa SQL nghiệp vụ. Đăng ký tường minh trong `generate_handler!`.
- `services/`: invariant kinh doanh — validate settings, ownership chapter→manga trong transaction, phân biệt not-found, chuẩn bị storage root (`covers/`, `cache/`) nhưng **không** mở đường dẫn tùy ý.
- `db/repositories/`: SQLx runtime query (không macro `query!` — tránh DATABASE_URL build-time), upsert idempotent theo `(source_id, source_manga_id)` / `(manga_id, source_chapter_id)`, pagination có giới hạn + escape LIKE.
- `domain/`: typed DTO + error-safe helpers; `error.rs`: AppError → wire `{code, message, retryable}` camelCase, message tĩnh đã sanitize.
- `AppState { db: SqlitePool }` quản lý một lần trong `setup` (block_on init → fail sớm, health không bao giờ giả `ready`).
- `db/mod.rs`: một pool duy nhất, foreign_keys ON, busy_timeout 5s, **không WAL** (quyết định Phase 01), migrations nhúng, forward-only.

## Tầng frontend

- Service interfaces (`src/services/*.service.ts`) là boundary duy nhất giữa màn hình và dữ liệu. `provider.ts` quyết định mock hay IPC-backed; Phase 05–07 thay implementation, screens không đổi.
- Demo vs Local tách biệt tường thuật (badges, empty-state copy).
- Routing HashRouter + `routes.ts` contract; state ephemeral của demo nằm trong mock store (`useSyncExternalStore` qua `useServicesVersion`).

## Bảo mật

- Capability: `core:default` + 5 window permission cụ thể, window `main` duy nhất. App-defined commands không cần permission.
- CSP: `script-src 'self'`, style `'unsafe-inline'` (React inline styles), không wildcard, không remote.
- Không shell/fs/http plugin, không generic fetch/SQL/path command. DB path chỉ từ `app.path().app_data_dir()`.
- Không log/IPC nào lộ SQL, đường dẫn tuyệt đối của user, stack trace.

## Migration policy

`app_settings` Phase 01 giữ nguyên; schema Phase 04 = `0002_domain.sql` (xem `docs/phase-04/schema.md`). Forward-only, kiểm thử fresh + upgrade + reopen. Phase tiếp theo thêm số thứ tự mới, không sửa migration đã phát hành.
