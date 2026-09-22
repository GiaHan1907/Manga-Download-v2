# Phase 03 — Core UI Screens & Interactive Prototype: Implementation Notes

**Ngày:** 2026-09-22 · **Implementer:** Qoder agent · **Reviewer:** Codex
**Trạng thái:** P03-01 audit & contracts. Kết quả lệnh/test được bổ dần ở mục 6 khi từng task hoàn thành.

## 0. Audit repository (P03-01)

### Tài liệu tham chiếu

- `README.md` (Phase 01+02), `AGENTS.md`, `docs/phase-02/implementation-notes.md`, git history (4 commits, HEAD `ea0f05e`) — đã đọc.
- **Deviation:** brief §2.1 yêu cầu đọc `docs/ARCHITECTURE.md` và `docs/IPC_CONTRACT.md` — **hai file này không tồn tại trong repo**. Nguồn sự thật hiện hành: README + Phase 02 notes + code. Không tạo file giả; ghi nhận tại đây.
- Working tree sạch trước khi sửa; `AGENTS.md` không bị đụng tới ở Phase 01–02 và tiếp tục được bảo tồn.

### Reuse từ Phase 02 (giữ nguyên, không rework)

- Layout: `AppShell`, `WindowTitlebar`, `Sidebar`, `Topbar`, `PageContainer`; tokens Dark Purple trong `src/styles/tokens.css`.
- UI primitives: `Button` (native/link), `Input`, `Dialog`, `Tabs`, `Tooltip`, `Progress`, `StatusBadge`, `Skeleton`, `EmptyState`, `ErrorState`, `MangaCard`, `ChapterRow`, `DownloadCard`.
- IPC: `src/shared/api/desktop.ts` (chỉ `app_get_info`, `app_health_check`) — **Phase 03 không thêm command Rust nào**; `SystemStatus` giữ logic health thật.
- Routing: `HashRouter` trong `src/app/router.tsx`; `src/app/routes.ts` là hợp đồng route/sidebar.
- Scripts: npm (lint/typecheck/test/build/tauri) + cargo. Playwright **không** được cấu hình → dùng Vitest + Testing Library cho integration test (brief §7 cho phép).
- Window minimum: Tauri config `1024×700` (Phase 02) khớp yêu cầu Phase 03.

### Missing / cần bổ sung (Phase 03 tự dựng, frontend-only)

1. `src/domain/` typed models; `src/services/` contracts + `src/services/mock/` (fixtures, store, provider). Brief §2.4 nói "React Query if already configured; Zustand or existing solution" — **không có sẵn**; thêm 2 state library mới trái với rule "avoid parallel sources of truth" + AGENTS.md "no unnecessary dependencies". **Quyết định:** external store tối giản (subscribe/notify) + `useSyncExternalStore` — một chủ sở hữu state duy nhất, đổi được sang IPC-backed mà không rework screen. Ghi là deviation có giải trình.
2. Screens feature-based (`features/home|discover|manga-detail|library|downloads|reader|settings`) thay placeholder; route `/manga/:mangaId`, `/reader/:mangaId/:chapterId`, `/settings/sources` (2 route demo Phase 02 không giữ được vì Phase 03 cần param; sidebar routes giữ nguyên).
3. Sample reader images: tự sinh **trung tính, original** (SVG shape + số trang) tại `src/assets/sample-reader/` — không dùng ảnh bên thứ ba.
4. Badge "Demo · dữ liệu mẫu" vĩnh viễn trong Topbar (§2.5).

### Route map

| Route | Screen | Data |
|---|---|---|
| `/` | Home Dashboard | mock services (stats, continue reading, recent, queue preview) + Phase 01 SystemStatus |
| `/discover` | Discover: search + source filter + tab `?tab=url` import | CatalogService |
| `/manga/:mangaId` | Manga Detail: hero, chapters, select/queue/favorite | Catalog + Download + Library |
| `/library` | Library grid/list | LibraryService |
| `/favorites` | Favorites | LibraryService (cùng state) |
| `/downloads` | Demo queue + transitions | DownloadService |
| `/reader/:mangaId/:chapterId` | Sample Reader preview | Reader fixtures + progress state |
| `/settings` | Demo settings (validation, save/reset trong mock state) | Settings state |
| `/settings/sources` | Nguồn demo enable/disable; add-source disabled | Catalog sources state |
| `*` | 404 trong shell | — |

### Mock state diagram

```
fixtures.ts (bất biến, deterministic, không Math.random/Date.now)
   │ seed (deep clone)
   ▼
store.ts ── subscribe/notify, version↑, reset()            ← test helper (dev-only)
   ▼
mock/*.ts services (async contract, validate transitions,
   │        dedupe queue, favorite toggle, progress write)
   ▼
provider.ts (mỗi screen nhận qua useServices())
   ▼
screens (useServices + useSnapshot/useAsyncData — không import fixtures trực tiếp)
```

State khả biến: `tasks[]`, `favorites: string[]`, `libraryAdded: string[]`, `readingProgress: Record<mangaId,{chapterId,page}>`, `enabledSources: string[]`, `settings`.

### Interaction matrix (control chính → hành vi thật)

| Screen | Control | Hành vi prototype |
|---|---|---|
| Home | Explore Manga | navigate `/discover` |
| Home | Import URL | navigate `/discover?tab=url` |
| Home | Continue Reading | open reader đúng chapter/page đã lưu |
| Discover | search input | debounce 300ms, filter title/author/genre, guard stale result |
| Discover | source select | lọc theo nguồn đang bật |
| Discover | import demo URL | match fixture `demo.manga1989.example` → preview → Detail; URL khác → lỗi diễn giải được; **không network** |
| Detail | select visible / clear | selection state, đếm số |
| Detail | Add selected to demo queue | queue mới, skip đã có; báo added/skipped |
| Detail | Favorite | toggle, đồng bộ Library/Favorites/Home |
| Detail | Read | chỉ chapter `hasSamplePages`; còn lại disabled + reason |
| Library | remove | Dialog xác nhận, đổi mock state, không xoá file |
| Downloads | Pause/Resume/Retry/Cancel | chỉ hiện khi transition hợp lệ; invalid bị service từ chối |
| Downloads | View manga | navigate Detail |
| Reader | prev/next/zoom/mode/direction | hoạt động; progress ghi vào mock state; Escape exit |
| Settings | save/reset | mock state; validation range (1–5, 1–8) |
| Settings | add source | **disabled** + lý do "Available after Source Engine integration" |

### Quy tắc chống-giả-mạo đã chốt

- Không MB/s, không "saved to disk", không Open folder; progress chỉ page-count và gắn nhãn `Simulated demo`.
- Stats Home chỉ từ fixture fields; storage hiển thị `—` / `Sample statistics`.
- Favorite/queue/progress là mock state, persist tối đa trong phiên (reset khi reload) — ghi rõ trong About.
- Transition hợp lệ (A — deterministic, preferred của brief §3.5): `queued↔paused`, `failed→queued (Retry)`, `queued|paused|failed→cancelled`, `completed` không transition. Không timer giả lập chạy nền.

## 1. P03-02 — Models, fixtures, service boundary

- `src/domain/{manga,download,reader}.ts`: typed models + discriminated transitions (`applyDownloadAction`, `allowedActions`), reader zoom/page clamps.
- `src/services/{catalog,library,download,reader,settings}.service.ts`: read/write **interfaces only**. Screens never import fixtures.
- `src/services/mock/fixtures.ts`: single deterministic catalog — 8 manga / 2 demo sources / mixed genres+statuses, chapters per manga, `SEED_TASKS` in completed/failed/queued/paused, fixed ISO timestamps, **no `Math.random()`/clock**. Reader chapter `manga-001-ch-01` has 4 original SVG pages; covers use a local placeholder gradient.
- `src/services/mock/store.ts`: single mutable-state owner (`subscribe`/`getMockVersion`/`resetMockState`) → React via `useSyncExternalStore` (`src/shared/hooks/useServices.ts`). Async `useAsyncData` wrapper with alive-flag cleanup (no dangling timers). `provider.ts` selects mock impls; Phase 04–07 swap = change `services` object only.
- Dev-only reset: `resetMockState()` (used by tests), **no misleading end-user reset button**.

## 2. P03-03 → P03-08 — Screens

- **Home** `features/home/HomePage.tsx`: quick links (Explore→/discover, Import→/discover?tab=url), stats derived only from fixture fields (storage = `—` "Not connected in UI prototype"), Continue Reading (opens saved `/reader/:mangaId/:chapterId`), Recent Additions, Queue Preview, + reused `SystemStatus` (Phase 01 health). loading/empty/error/long-title handled.
- **Discover** `features/discover/DiscoverPage.tsx`: `useDebouncedValue` 300ms search, accent-insensitive match, source filter (respects disabled sources), grid/list cards → `/manga/:id`; URL Import tab reads `?tab=url`. Import matcher classifies malformed vs unsupported vs unknown (no network). Uses `Tabs` (Phase 02) + `useMemo` guard against out-of-order results.
- **Manga Detail** `features/manga-detail/{MangaDetailPage.tsx,components/ChapterSelectRow.tsx}`: hero 2:3 cover, chapters search/sort(stable)/select-visible/deselect, `Add selected to demo queue` reports added/skipped, favorite toggle, `Read sample` only for `hasSamplePages` (others disabled + tooltip reason). Missing id → recoverable error, unknown → NotFound.
- **Library / Favorites** `features/library/{LibraryPage,FavoritesPage}.tsx`: grid/list, title/genre/status filters, sort, `Read` gated on sample availability, `Remove from sample library` via confirmation `Dialog` (mock-only). Favorites share the same store.
- **Downloads** `features/downloads/DownloadsPage.tsx`: All/Queued/Paused/Completed/Failed filter chips, title search, sort queue/newest, page-count `Progress` labeled simulated (no MB/s), transitions only for allowed actions (`allowedActions`), invalid rejected by service with message. `View manga` → detail; **no** Open folder.
- **Reader** `features/reader/{ReaderPage.tsx,components/ReaderImage.tsx}`: `Sample Reader` label, single/double/vertical, LTR/RTL order, zoom w/ clamp + keyboard, fullscreen/exit via existing UI (Escape), progress persisted to mock store and reflected on Home, per-image `onError` fallback, listener cleanup on unmount, `isTypingTarget` guard so shortcuts don't fire while typing.
- **Settings / Sources** `features/settings/{SettingsPage,SourcesPage}.tsx`: typed demo settings with range validation (1–5, 1–8) saved/reset only in mock state; output format labeled planned; About shows **real** app version via Phase 01 IPC; storage "Not connected". Sources toggles feed Discover filter; add-source **disabled** with reason.
- Cross-screen: added `StatusBadge` demo marker to Topbar (`Demo · dữ liệu mẫu`); route map extended in `src/app/routes.ts` with `sources` route; router replaces all Phase 02 placeholders (placeholder file deleted).

## 6. Kết quả lệnh / test

| Gate | Lệnh | Kết quả |
|---|---|---|
| Lint | `npm run lint` | pass — 0 errors |
| Typecheck | `npm run typecheck` | pass |
| Frontend tests | `npm run test` | pass — 44 tests / 4 files (mockServices 21, app 5, windowService 2, phase03 integration 15, +1) |
| Frontend build | `npm run build` | pass — 1952 modules, 356 kB JS (gzip 109.7 kB) |
| Rust check | `cargo check` | pass — unchanged Phase 01/02 (1 dead_code warning `Internal`) |
| Rust tests | `cargo test` | pass — 2/2 DB tests (regression intact) |
| `npm run tauri dev` smoke | recorded §7.1 | pass — clean launch + exit 0, no runtime errors |
| `npm run tauri build` | not run | slow; run before release |

## 7. Test matrix T03 → evidence (automated where possible)

| ID | Automated in | Result |
|---|---|---|
| T03-01 shell + health | app.test.tsx (downloads route, active sidebar), phase03 (badge) | pass (health IPC mocked truthful; real DB init unchanged in Rust) |
| T03-02 Home quick actions | phase03.test.tsx | pass |
| T03-03 search + filter | phase03.test.tsx | pass |
| T03-04 URL validation | phase03.test.tsx + mockServices.test.ts | pass (no network path exists) |
| T03-05 chapter selection/queue | phase03.test.tsx (added/skipped) | pass |
| T03-06 cross-screen sync | phase03.test.tsx (favorite→Favorites, remove dialog, reader→Home) | pass |
| T03-07 download transitions | phase03.test.tsx + mockServices.test.ts | pass (invalid transitions rejected) |
| T03-08 reader controls | phase03.test.tsx (bounds, progress restore, fallback) | pass |
| T03-09 empty/error/404 | app.test.tsx 404, phase03 missing manga/reader | pass |
| T03-10 keyboard/a11y | Tabs/arrows in code; Dialog Escape tested; reader shortcuts implemented | code-complete; full manual keyboard sweep pending on desktop |
| T03-11 layout/scaling | min 1024×700 configured; CSS wraps | needs desktop screenshots at 100/125/150/200% (not captured here) |
| T03-12 Rust regression | cargo test/check | pass |
| T03-13 build + smoke | npm run build, tauri dev | build pass; dev recorded §7.1 |

### 7.1 Windows smoke (tauri dev)

`npm run tauri dev` (2026-09-22): Rust compile 22.35s (chỉ warning `Internal` chủ đích), app launch `target\debug\manga-downloader-1989.exe` với Phase 03 screens, đóng cửa sổ exit code **0** — không panic, không lỗi module, không permission denial. Phase 03 không đổi Rust/config nên kết quả này đồng thời xác nhận regression T03-12 ở tầng process. Visual/keyboard sweep có chủ đích của người dùng trên hành trình demo (Home→Discover→Detail→Queue→Downloads→Library→Reader→Settings) và scaling 125–200% vẫn được khuyến nghị khi review; chưa chụp screenshot vào docs.

## 8. Mock → Phase 04–07 backend swap map

| Interface (frontend) | Phase 03 mock | Future backend |
|---|---|---|
| `CatalogService` | `mockCatalogService` | Rust adapter commands (`source_search`, `source_resolve_url`, `manga_get`) via `desktop.ts` |
| `LibraryService` | `mockLibraryService` | SQLite tables + commands |
| `DownloadService` | `mockDownloadService` | Downloader queue + real `download` progress events (must NOT reuse mock states) |
| `ReaderService` | `mockReaderService` | On-disk CBZ/folder reading through fs plugin (scoped) |
| `SettingsService` | `mockSettingsService` | `app_settings` table (key/value_json) |

Swap = replace `services` object in `provider.ts`; screens unchanged. Reset helper `resetMockState` becomes test-only.

## 9. Open issues / limitations

1. Fixtures are session-only; reload resets state (expected for prototype).
2. Reader images are original neutral SVGs — real content requires Phase 04 assets.
3. Real Windows visual checks (scaling 125/150/200%, 1024×700 no-clip, reduced-motion) need a desktop session; not captured headless.
4. `npm run tauri build` not run here — required before release.
5. Playwright not configured → E2E journey covered by RTL integration test instead.
6. Icon remains Phase 02 16×16 placeholder (release item).
