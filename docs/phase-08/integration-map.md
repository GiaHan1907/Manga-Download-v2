# Phase 08 — Integration Map (P08-01)

Audit baseline: commit `0f81dac` (Phase 07 + review 3 vòng). Mọi đường dẫn dưới đây là code THẬT đã kiểm tra bằng read-only audit, không suy đoán.

## 1. Route → Component → Frontend API → IPC → Rust owner → Storage

| Route | Component | Dữ liệu | IPC commands | Rust owner | Bảng/file owner | Trạng thái |
|---|---|---|---|---|---|---|
| `/` | `features/home/HomePage.tsx` | MIXED: panel demo (mock `services.*`) + `LocalContinueReading` THẬT + `SystemStatus` THẬT | `app_get_info`, `app_health_check`, `library_list` | system_service, library_service | `mangas.last_read_at`, `reading_progress` | ⚠ demo panel còn chiếm vị trí stats |
| `/discover` | `discover/DiscoverPage.tsx` | tab "Tìm kiếm (thật)"/"Import URL (thật)" + tab Demo sandbox | `search_manga`, `resolve_manga_url`, `get_remote_manga_details`, `save_discovered_manga` | discovery_service + SourceRegistry | `sources`, `mangas`, `chapters` | ✅ real; ⚠ tab demo |
| `/manga/:mangaId` | `manga-detail/MangaDetailPage.tsx` | 100% MOCK | — | — | — | ⚠ dev-sandbox, không reachable từ luồng thật |
| `/remote/:sourceId/:sourceMangaId` | `discover/RemoteDetailPage.tsx` | REAL | như Discover | discovery_service | `mangas/chapters` upsert | ✅ |
| `/library` | `library/LibraryPage.tsx` | REAL | `library_list`, `app_set_manga_favorite`, `library_reconcile` | library_service | `mangas/chapters/download_tasks/favorites/reading_progress` aggregate | ✅ |
| `/library/:mangaId` | `library/LocalMangaPage.tsx` | REAL | `app_get_local_manga`, `library_get_downloaded_chapters`, `queue_chapter_downloads`, `library_remove_chapter`, `library_remove_manga_files` | catalog/library/downloader | `chapters.local_path`, `download_tasks`, file `library/` | ✅ |
| `/favorites` | `library/FavoritesPage.tsx` | REAL, **không pagination** (limit 20 cứng) | `app_list_favorite_mangas`, `app_set_manga_favorite` | favorites_service | `favorites` | ⚠ |
| `/downloads` | `downloads/DownloadsPage.tsx` (+`useDownloadQueue`) | REAL queue+history | `app_list_local_download_tasks` (limit **100**, offset 0), pause/resume/cancel/retry | download engine | `download_tasks/pages` | ⚠ cap 100, không pagination UI |
| `/settings` | `settings/SettingsPage.tsx` | REAL IPC nhưng **copy còn ghi "mock"** | `app_get_settings`, `app_update_settings`, `app_reset_settings` | settings_service (blob `settings.v1`) | `app_settings` | ⚠ wording; pdf option không engine nào hỗ trợ |
| `/settings/sources` | `settings/SourcesPage.tsx` | REAL CRUD + toggle | `list/add/remove/set_source_enabled` | SourceRegistry | `sources` | ✅ |
| `/reader/:mangaId/:chapterId` | `reader/ReaderPage.tsx` | 100% MOCK | — | — | — | ⚠ dev-only |
| `/read/:chapterId` | `read/ReadPage.tsx` | REAL | `reader_open_chapter`, `reader_get_page_bytes`, `app_get/set/save_reading_progress`, `app_get_settings` | reader/library/reading services | file `library/` + `reading_progress` | ✅ |
| `*` | NotFoundPage | — | — | — | — | ✅ |

## 2. Cơ chế refresh hiện có (quan trọng nhất cho P08-02)

- `useServicesVersion` (`shared/hooks/useServices.ts`) → **chỉ bump khi MOCK store đổi**; dữ liệu thật không có invalidation liên màn.
- Mỗi màn tự `retryNonce/nonce` sau mutation → chỉ refresh CHÍNH nó; quay lại màn khác phải remount mới thấy dữ liệu mới.
- Event `download-task-updated` chỉ được `DownloadsPage` subscribe qua `useDownloadQueue` (revision-guarded). Không có listener cấp app.
- **Quyết định Phase 08 (deviation có chủ đích khỏi gợi ý TanStack/Zustand trong plan §2.6):** KHÔNG thêm dependency state mới (plan §2/§4 ưu tiên cấu trúc hiện hữu, AGENTS.md cấm dep thừa). Thay bằng invalidation bus nội bộ `src/shared/hooks/useInvalidation.ts` (external store + domain keys đúng bộ `qk` gợi ý: home/library/favorites/downloads/progress/settings/sources) + `AppEventBridge` mount 1 lần trong shell: subscribe `download-task-updated`, bump domain theo terminal transition, giữ active-count cho badge.

## 3. Settings keys thực tế (blob `settings.v1`, validate Rust `domain/settings.rs`)

| Key | Range | Default | Consumer thật |
|---|---|---|---|
| defaultReaderMode | single/double/vertical | single | ReadPage seed |
| defaultReadingDirection | ltr/rtl | rtl | ReadPage seed |
| chapterConcurrency | 1..=5 | 2 | scheduler dispatch |
| imageConcurrency | 1..=8 | 4 | worker per-image |
| outputFormat | cbz/folder/**pdf** | cbz | queue default; **pdf bị engine từ chối tại queue** (`downloads.rs:52-56`) |

Phase 08 thêm: `notificationsEnabled: bool` (default true), `closeToTray: bool` (default false) — consumed bởi Rust notification/tray; UI Settings bỏ option `pdf` (ẩn hẳn theo T08-18).

## 4. Storage/cache roots

- `library_root = <app_data>/library` — verified outputs, KHÔNG bao giờ là target cleanup.
- `staging_root = <app_data>/cache/downloads/<task-id>/` — regenerable. Lỗ hổng hiện tại: staging của task **cancelled** hoặc task không còn trong DB không bao giờ bị dọn (recovery chỉ quét task có DB status unfinished; worker cancel path để lại dir).
- `download_pages` rows của terminal tasks giữ vĩnh viễn — **không** phải target clear-cache (checkpoint semantics).
- Phase 08 thêm: `storage_status` (size library từ DB SUM + size cache từ fs walk, chỉ số approximate được khai rõ), `clear_regenerable_cache` (chỉ staging dir của task absent/completed/cancelled, chặn mọi id còn active), dọn cùng lúc ở startup recovery.
- Library root: **không cho đổi** ở Phase 08 (tránh orphan/move file nửa vời — plan §5.6 cho phép defer khi chưa có migration flow); "Open folder" cần capability shell/opener mới → **deferred có ghi chú**, UI hiển thị nhãn tương đối, không lộ absolute path qua IPC.

## 5. Windows integration hiện trạng → target

- Hiện tại: 0 plugin, capabilities = core:default + 5 window perms; close X = ExitRequested → shutdown ≤5s → exit; task active về queued qua recovery lần sau. Không tray, không notification, không global shortcut.
- Target Phase 08: `tauri-plugin-notification` (Rust side; capability `notification:default`) thông báo theo chapter-terminal (completed/failed), tôn trọng `notificationsEnabled`; tray (`tauri` feature `tray-icon`) menu Show/Hide · Open Downloads · Quit; `closeToTray=true` → X ẩn cửa sổ (download tiếp tục vì process sống), Quit từ tray = exit thật qua lifecycle. **Không** thêm global-shortcut OS-level (Ctrl+K chỉ trong app, tránh plugin thừa).

## 6. Mock leakage còn lại (P08-08 phải xử lý)

1. HomePage: hero/StatCards/panel "Thêm gần đây"/demo queue từ `src/services/mock`. → thay bằng số THẬT (stats qua `library_list` total + aggregate mới) + cách ly demo.
2. Tab "Demo sandbox" Discover, route `/manga/:id`, `/reader/:manga/:chapter` → gate `import.meta.env.DEV` (dev-only, production build không render — plan §2.7 "cách ly, không hiển thị production").
3. Copy Settings ghi "mock" trên đường thật → sửa.
4. `provider.ts` dual-boundary (`services` mock vs `localServices` real): giữ nguyên seam cho sandbox dev, không unify tốn churn; production screens chỉ đi `localServices`.

## 7. Test surface hiện có

- Rust: 101 test inline `#[cfg(test)]` + `downloader/engine_tests.rs`; convention `tNN_NN_*`; DB qua `crate::db::test_pool()`, fs qua `tempfile`.
- Frontend: vitest 9 files (`tests/phase0X.test.tsx` mock `invoke` theo switch command); pattern renderAt + MemoryRouter.
- Phase 08 tests mới theo T08-matrix: invalidation bridge reducer, palette (keyboard/offline/XSS), favorites pagination, downloads pagination, storage status/clear-cache (Rust), settings keys mới (Rust), progress fallback (Rust), regression Phase 06/07.

## 8. Kết luận audit

Không cần migration mới (settings là blob JSON; aggregate dùng SQL trên schema 0002–0005; không FTS cho local search quy mô desktop). Command mới tối hạn: `library_stats`, `storage_status`, `clear_regenerable_cache`. Capability mới duy nhất: `notification:default`. Dep mới: `tauri-plugin-notification` (Rust) + feature `tray-icon` — frontend package.json không đổi.
