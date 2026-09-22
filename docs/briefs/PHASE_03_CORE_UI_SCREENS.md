# MANGA DOWNLOADER 1989 — PHASE 03 IMPLEMENTATION PLAN

**Phase:** 03 — Core UI Screens & Interactive Prototype  
**Platform:** Windows 10/11 desktop  
**Stack:** React + TypeScript + Vite + Tauri 2 + Rust + SQLite (reuse Phase 01–02 foundation)  
**Theme:** Dark Purple (reuse Phase 02 design tokens)  
**Implementer:** Antigravity · **Reviewer:** Codex  
**Prerequisites:** Phase 01 and Phase 02 have passed review; inspect the **actual** repository before making changes.  
**Phase result:** A complete, navigable UI prototype with deterministic, explicitly marked mock data. **Not** a working manga downloader.

## 1. Objective and scope

Build all product-facing screens on the existing Tauri AppShell so a user can follow the full prospective journey: Discover / supported-example URL → Manga Detail → select chapters → mock download queue → Library → offline Reader preview. Implement functional UI interactions and coherent mock state, while keeping data adapters separate from presentation so Phase 04–07 can replace them with actual Rust/SQLite services.

**In scope:** Home Dashboard, Discover/Search + URL Import, Manga Detail, Library, Downloads, Reader preview, Favorites, Settings + Sources presentation; shared typed domain models; feature APIs backed by fixtures; loading/empty/error states; route and interaction tests; Windows smoke test; UX handoff.

**Out of scope:** real source adapters or arbitrary URL fetching; real search/import/network download; real progress/speed/CBZ/PDF; filesystem reading or library persistence through SQLite; backend schema changes and new unrestricted Tauri permissions; production updater. Those belong to later phases. Do not claim mock features are real. **Do not implement downloader or reader backend in Phase 03.**

## 2. Mandatory architecture and working rules

1. Read `README.md`, `AGENTS.md`, `docs/ARCHITECTURE.md`, `docs/IPC_CONTRACT.md`, Phase 01 handoff, Phase 02 implementation notes, current router, theme, components, state and test scripts. Check `git status` before editing. Do not re-scaffold, replace a working package manager, rename existing routes arbitrarily or overwrite user work.
2. Reuse Phase 02 sidebar, titlebar, topbar, tokens, `Button`, `Input`, `Dialog`, `Tabs`, `Progress`, `StatusBadge`, `MangaCard`, `ChapterRow`, `DownloadCard`, `Skeleton`, `EmptyState`, `ErrorState`. Extend a component only when a documented screen requirement needs it; centralize styling.
3. **Screens consume feature services**, not direct fixture imports. Define read/write interfaces now; provide a `mock` implementation in Phase 03. Later swap it for an IPC-backed implementation without reworking route components.
4. Keep React Query for asynchronous read models if already configured; use Zustand or existing solution for ephemeral UI and mock mutations. Avoid parallel sources of truth. Define a predictable mock reset for tests. Never reuse `download` events from Rust for fake progress.
5. Maintain stable identifiers for fixture Manga, chapters and tasks across all screens. Seed data includes titles/authors/generic sample covers with legitimate local placeholders; do not use copyrighted third-party images without permission. Clearly label the app as **Demo / sample data** in this phase (e.g. persistent unobtrusive badge in AppShell).
6. Mock URL parsing may accept a documented fictitious domain such as `https://demo.manga1989.example/manga/manga-001` **only as locally matched fixture text**. No network call, redirect, scraping, generic fetch, or misleading claim that arbitrary website URLs work. Reject unsupported/invalid URLs with actionable feedback.
7. Every visible primary control must do something meaningful **within the prototype** (navigate, update mock state, show a real local preview, apply filters, or open a meaningful dialog). A future-only action must be visibly disabled with explanation, not clickable and inert. Never show fake MB/s, real download success, actual CBZ exports, or 'saved to disk' confirmations.
8. Preserve Phase 01 Rust/SQLite bootstrap and Phase 02 Tauri window capabilities. Phase 03 is frontend-focused; do not add shell, broad filesystem or network permissions. Fail visibly if existing health IPC fails; never hardcode database readiness.
9. Accessibility: keyboard navigation, semantic controls, labeled inputs, visible focus, modal focus management, screen reader status announcements, reduced-motion support, sufficient contrast and usable layouts at min 1024×700 and Windows 100/125/150/200% scaling (document which scales were actually tested).
10. Code strict TypeScript with discriminated unions for task status; avoid `any`, dangling timers, direct DOM manipulation for view state, and mock side effects in module initialization. Responsive behavior must retain access to all key actions, no horizontal screen overflow.

## 3. Screen specifications and interaction contracts

### 3.1 Home `/`
- Header and quick actions: `Explore Manga` → `/discover`; `Import URL` opens the Discover URL dialog or navigates to `/discover?tab=url`.
- Dashboard cards: total manga, demo chapters, sample storage **only when accurately derived from fixture fields**; do not invent real disk usage. If unavailable show `—` / `Sample statistics`.
- `Continue Reading`: most recently read fixture item, cover, chapter/page, progress; `Continue` opens `/reader/:mangaId/:chapterId` with correct mock page. If no history show EmptyState linking to Library.
- `Recent Additions` and `Download Queue`: use shared fixture state; clicking a manga opens its Detail; clicking a task opens Downloads.
- States: populated, empty, loading, recoverable error; at 1024×700 dashboard cards wrap without cropping.

### 3.2 Discover `/discover`
- Search text, source selector (all/enabled mock sources), searchable results in cover grid/list, clear filters, no-results state, keyboard-friendly search.
- Query debounce ~300 ms; out-of-order async results must not overwrite newest query. Source filter and query are reflected in the UI; URL query parameters optional but must be tested if implemented.
- Import URL tab/dialog: trim input, syntactic URL validation, match only documented fixture URLs, show mock source/manga preview and route to its Detail; invalid/unsupported URLs show distinct errors. Label clearly `Demo import — supported sample URL only`.
- Search result card navigates to `/manga/:mangaId` and preserves source identity. Cover loading failure falls back to a styled placeholder.

### 3.3 Manga Detail `/manga/:mangaId`
- Hero: 2:3 cover, title, author, description, genres, publication status, demo source identifier, optional metadata without fabricated release dates.
- Chapters: search/filter and order, select one/multiple/all **visible** chapters, deselect all, selection count, badges reflecting mock queue/library state. Stable sort for chapters with equal/absent chapter numbers; title and source chapter IDs remain intact.
- `Add selected to demo queue`: adds only not-yet-queued selected chapters to a coherent in-memory mock queue; reports added/skipped counts, preserves selection on recoverable errors. No actual download occurs.
- `Favorite` toggles mock favorite state and updates Library/Favorites/Home immediately. `Read sample` works only for chapters with fixture pages; others show disabled state with reason.
- Missing/unknown manga ID → NotFound; zero chapters → EmptyState; long description/long titles → layout remains usable.

### 3.4 Library `/library` and Favorites `/favorites`
- Library grid/list switch; filter title, genre, status, mock `available offline` flag; sort by title/recently added/progress. Responsive 2:3 cover cards and keyboard-accessible action menus.
- `Open Detail`, `Read` (only for demo chapters with local sample pages), favorite toggle; `Remove from sample library` via confirmation dialog only changes mock state (no filesystem deletion).
- Favorites displays items from the same shared state and supports removing a favorite; empty state offers `Discover` action. Search/filter consistency after navigation or reset.

### 3.5 Downloads `/downloads`
- Header summary of **demo** tasks; filters `All / Queued / Paused / Completed / Failed`, search by title, sort by queue order/newest.
- Task card: manga title, chapter, status, page-count-based **simulated demo** progress (not speed/bytes), accessible progress semantics, clear demo marker. States via explicit transitions (queued ↔ paused, failed → queued by Retry, queued/paused → cancelled by Cancel if implemented).
- Choose one approach: (A) deterministic fixture tasks with immediate user-driven status transitions, **preferred**, or (B) optional bounded test-only progress simulation with explicit `Simulated` label and proper timer cleanup. Never imply real images were transferred, output files written, or downloads completed by Rust.
- Buttons: `Pause`, `Resume`, `Retry`, `Cancel` appear only when valid for current state; invalid transitions are rejected by mock service. Completed demo cards can show `View manga`, **not** `Open folder` or `Read downloaded file` unless actual sample assets exist.
- No dead controls or fake network speed. Empty/failed/error variants; state remains consistent after route changes.

### 3.6 Reader preview `/reader/:mangaId/:chapterId`
- Explicit `Sample Reader` label; render bundled **authorized sample images or generated neutral illustrations**, not third-party copyrighted manga pages. At least one sample chapter has 3+ images with stable page IDs.
- Single-page, double-page, and vertical layout; navigation next/previous; LTR/RTL page order as selected; zoom with min/max; fit width/fit page; fullscreen using existing Phase 02 window service or browser-supported safe UI behavior; exit via Escape.
- Keyboard shortcuts only while Reader is active and not typing in an input/dialog. Visible current page/total. Page navigation respects boundaries; chapter selector only shows chapters with sample pages.
- Store demo reading progress in shared mock state when page/chapter changes; Continue Reading on Home reflects it. Prefetch only adjacent sample pages if useful; cleanup listeners on route exit. On missing image render localized fallback instead of blank screen.
- This is a **reader UI prototype**, not CBZ parsing or real offline import.

### 3.7 Settings `/settings` and Sources `/settings/sources` (or preserve existing Phase 02 route)
- Settings groups: General, Downloads, Reader, Sources and About. Reuse UI components and Phase 02 theme tokens.
- Demo settings: theme preview if existing switch supports it; default reader mode, direction, chapter concurrency (1–5), image concurrency (1–8), output format selector (CBZ/Image Folder/PDF **planned, not actually exported**). Form validation and save/reset within mock state; do not write misleading backend preferences.
- Sources list shows `Demo source` with status and capability badges; enable/disable modifies mock Discover source filter. Add-source UI is disabled with `Available after Source Engine integration` rather than pretending adapter installation exists.
- Storage settings display `Not connected in UI prototype`; no browse/permission grants unless previously implemented and actually functional.

## 4. Domain types and mock service contract

The following is a guideline; **adapt to existing Phase 01–02 type definitions** and avoid duplicate types/route conventions:

```ts
export type MangaStatus = "ongoing" | "completed" | "unknown";
export type DownloadStatus = "queued" | "paused" | "failed" | "completed" | "cancelled";

export interface MangaSummary {
  id: string;
  sourceId: string;
  sourceMangaId: string;
  title: string;
  author?: string;
  coverAsset?: string;
  genres: string[];
  status: MangaStatus;
}

export interface MangaDetail extends MangaSummary {
  description: string;
  chapters: ChapterSummary[];
}

export interface ChapterSummary {
  id: string;
  mangaId: string;
  title: string;
  number?: number;
  pageCount?: number;
  hasSamplePages: boolean;
}

export interface DemoDownloadTask {
  id: string;
  mangaId: string;
  chapterId: string;
  status: DownloadStatus;
  completedPages: number;
  totalPages: number;
  createdAt: string; // deterministic ISO fixture timestamp
}

export interface MangaCatalogService {
  search(query: string, sourceIds: string[]): Promise<MangaSummary[]>;
  resolveDemoUrl(url: string): Promise<MangaSummary>;
  getManga(id: string): Promise<MangaDetail | null>;
  getChapters(mangaId: string): Promise<ChapterSummary[]>;
}

export interface DemoLibraryService {
  listManga(): Promise<MangaSummary[]>;
  toggleFavorite(mangaId: string): Promise<boolean>;
  listFavorites(): Promise<MangaSummary[]>;
}

export interface DemoDownloadService {
  queueChapters(chapterIds: string[]): Promise<{ added: number; skipped: number }>;
  listTasks(): Promise<DemoDownloadTask[]>;
  transition(taskId: string, action: "pause" | "resume" | "retry" | "cancel"): Promise<void>;
}
```

**Mock rules:** use a single deterministic fixture catalog (at least 8 mangas, 2 named mock sources, mixed genres/statuses, multiple chapters); at least 1 reader-enabled chapter with 3+ bundled sample images; include completed/failed/queued/paused example tasks without suggesting actual file downloads. Use fixture keys and timestamps fixed in source; never use `Math.random()` or current time in snapshot-dependent seed data. Maintain referential integrity between `mangaId`, `chapterId`, `taskId`; no duplicates after queuing. Tests must be able to reset mock state before each run. A single service factory/provider selects mock implementations; avoid leaking fixture imports into presentation components.

## 5. Target file layout (adapt existing repository; do not create empty placeholders)

```text
src/
├── app/
│   ├── router.tsx                    # replace Phase 02 route placeholders
│   └── providers.tsx                 # shared service / query / UI state
├── components/
│   ├── layout/                       # reuse Phase 02 AppShell, sidebar, topbar
│   ├── ui/                           # reuse Phase 02 primitives
│   └── manga/                        # reusable cover, chapter, task visuals
├── features/
│   ├── home/{HomePage.tsx,components/}
│   ├── discover/{DiscoverPage.tsx,components/,hooks/}
│   ├── manga-detail/{MangaDetailPage.tsx,components/}
│   ├── library/{LibraryPage.tsx,FavoritesPage.tsx,components/}
│   ├── downloads/{DownloadsPage.tsx,components/}
│   ├── reader/{ReaderPage.tsx,components/,hooks/}
│   └── settings/{SettingsPage.tsx,SourcesPage.tsx,components/}
├── domain/{manga.ts,download.ts,reader.ts}
├── services/
│   ├── catalog.service.ts           # interfaces / shared contract
│   ├── library.service.ts
│   ├── download.service.ts
│   └── mock/                        # mock provider, fixture catalog, state
├── stores/                          # single mock mutable-state owner
├── assets/sample-reader/            # licensed/original example images
└── tests/                            # feature and navigation tests

docs/phase-03/
├── README.md                         # routes, UI states, sample-data disclaimer
├── screen-checklist.md
└── implementation-notes.md           # actual commands, test results, review handoff
```

Keep existing file extensions/naming conventions; names above describe boundaries, not a request for destructive renames. Do not modify `src-tauri/migrations` unless strictly necessary for a Phase 01–02 regression fix agreed with reviewer.

## 6. Ordered implementation tasks

### P03-01 — Repository audit & screen contracts
- Inspect the current Git tree, Phase 01/02 handoff, router, all existing components and scripts. List reuse vs missing UI elements; confirm 1024×700 minimum and current theme variables.
- Write concise screen route map, mock-state diagram and interaction matrix; log conflicts/deviations before changing code.
- **Acceptance:** phase implementation notes identify actual existing files and show that Phase 01 health + Phase 02 shell still work.

### P03-02 — Typed models, mock fixtures and service boundary
- Implement domain interfaces, deterministic fixtures, resettable shared mock-state service/provider and async catalog/queue/library/reader adapter contracts.
- Seed coherent source–manga–chapter–task relationships and original sample images. Provide developer-only reset fixture action/test helper, not a misleading end-user reset button.
- Write unit tests: fixture referential integrity, search matching, duplicate chapter queue protection, task transition validation, favorite toggling, reader progress update.
- **Acceptance:** no `mock` imports in presentational components; demo data is stable, type safe and shared across routes; all related tests pass.

### P03-03 — Home Dashboard
- Replace Home placeholder; add accessible dashboard, Continue Reading, Recent Additions, Queue Preview and working quick links.
- Derive all displayed values from state; empty/loading/error and long-title handling.
- **Acceptance:** no invented file-size or real-download claims; clicking cards takes users to valid routes; reading progress updates Home.

### P03-04 — Discover, mock search and demo URL import
- Build search/filter UI, debounce/cancel out-of-order searches, result grid, import modal/tab and validation messaging.
- Add `demo.manga1989.example` fixture URL matcher; unsupported URLs are rejected without outbound requests.
- **Acceptance:** search, filter, empty/error/retry, URL preview and navigation to correct Manga Detail work via mouse and keyboard; no real web request.

### P03-05 — Manga Detail and chapter actions
- Build full hero, chapter list, search/sort/filter, multi-select and select visible, mock queue action, favorite and reader sample navigation.
- Make queue and favorite state visible in Library/Home/Downloads; deduplicate queue items; implement missing-record and empty-chapter states.
- **Acceptance:** select/unselect and queue added/skipped counts are correct; no phantom successes; no undefined chapter renders.

### P03-06 — Library, Favorites and Downloads
- Library Grid/List, filters, sorting and item menus; Favorites shares same state; implement confirm dialog for sample removal.
- Downloads reads shared mock tasks and supports valid UI-driven transitions, status filters and navigation back to manga detail.
- **Acceptance:** after queueing in Detail, Downloads updates; favorite changes synchronize immediately; task status remains stable when navigating away/back; no fake throughput, output files or Windows folder links.

### P03-07 — Reader preview
- Implement demo images, reader modes, previous/next, page indicator, zoom limits, keyboard controls, layout/reading direction, accessible fullscreen/exit and missing-page fallback.
- Save demo page position into shared state; reflect in Home; test change of manga/chapter and event cleanup on unmount.
- **Acceptance:** keyboard and click navigation stay within bounds; progress is preserved during route changes; no CBZ parsing is falsely implied.

### P03-08 — Settings, Sources and cross-screen polish
- Implement typed demo settings with validation, source toggle, About (real app version where available), and the required disabled states for future backend-only features.
- Resolve any navigation dead ends; add unified loading/empty/error/unauthorized/no-sample states; ensure modals and action menus work with keyboard.
- **Acceptance:** toggling source affects Discover; settings visibly labeled preview-only; every active control has an implemented local effect.

### P03-09 — UI, integration and Windows regression tests
- Add tests from the matrix below; run the actual existing project scripts. Run React lint/typecheck/Vitest tests, Rust checks from Phase 01 and Windows Tauri dev/build smoke tests.
- Capture actual screenshots for Home, Discover, Detail, Library, Downloads, Reader and Settings at 1440×900 and 1024×700, plus tested display scales. Check dark theme contrasts and reduced motion.
- Write `docs/phase-03/implementation-notes.md` with files changed, commands+results, screenshots, limitations, feature→backend swap mapping and open issues. Do not claim a check passed if it was not run.
- **Acceptance:** all mandatory test cases pass or exact environmental blockers are documented; Codex Reviewer receives reproducible handoff.

## 7. Mandatory testing matrix

| ID | Test | Expected result |
|---|---|---|
| T03-01 | App start and route reload | Phase 01 health still truthful; Phase 02 shell/titlebar/sidebar works; no blank routes |
| T03-02 | Home quick actions | Discover/Import open correct route/dialog; stats derived from mock state |
| T03-03 | Search and source filter | Matching results correct; no stale results; no-results state helpful |
| T03-04 | URL validation | Fixture URL resolves; malformed/unsupported URL fails safely; no network call |
| T03-05 | Detail chapter selection | Select all visible/deselect; order stable; queue added/skipped accurate |
| T03-06 | Cross-screen synchronization | Favorite/queue/reading-progress changes appear in relevant screens |
| T03-07 | Download transitions | Only allowed mock state changes; no invented MB/s or file success |
| T03-08 | Reader controls | Page bounds, modes, zoom, direction, shortcuts, progress restore and cleanup |
| T03-09 | Empty/error/404 | Missing manga/chapter, missing asset, empty catalog, simulated service error recover gracefully |
| T03-10 | Keyboard/a11y | Tab, Enter/Space, Escape, focus, labels, dialog focus, reduced motion |
| T03-11 | Layout and scaling | 1440×900 and 1024×700 usable; tested Windows display scales documented |
| T03-12 | Regression | Existing Rust check/tests, DB migration smoke and Tauri window controls remain functional |
| T03-13 | Build and artifact | Existing frontend build succeeds; Windows Tauri dev/build smoke outcome recorded honestly |

Suggested interaction E2E path (with Playwright if Phase 01/02 stack already supports it; otherwise React Testing Library integration tests): Home → Discover → Search → Detail → select chapter → Queue Demo → Downloads → Library → Reader Sample → change page → Home → Continue Reading. This tests **prototype UI behavior only**.

## 8. Commands and evidence

Use package manager and scripts already defined in `package.json`; do not silently introduce different tooling. Common commands **only if matching scripts exist**:

```powershell
npm ci
npm run lint
npm run typecheck
npm run test -- --run
npm run build
npm run tauri dev
npm run tauri build
```

For Rust checks, run from `src-tauri` (or existing workspace root):

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check
```

If commands differ or a toolchain is unavailable, record the **actual** command, exact exit status and unresolved validation, not fabricated green results. Preserve Phase 01 migrations and existing IPC health behavior.

## 9. Definition of Done / phase gate

- [ ] P03-01 → P03-09 implemented in order; differences from the plan documented.
- [ ] Home, Discover, Manga Detail, Library, Favorites, Downloads, Reader, Settings and Sources are accessible from real navigation with no dead primary actions.
- [ ] UI uses Phase 02 theme, typography, components and window shell consistently; layouts support min 1024×700, tested scaling and keyboard.
- [ ] Mock services are isolated behind typed feature APIs; fixture data is coherent, stable and clearly marked as demo; no false real download/reading/export status.
- [ ] The full demo journey works; invalid URL, missing records, empty catalog, bad sample asset and task transition errors are handled.
- [ ] Phase 01 app health/SQLite initialization and Phase 02 Windows shell still work; no broad security permissions added.
- [ ] Tests and build/check commands have been run where environment permits, with command outputs documented; platform-specific missing tests clearly listed.
- [ ] `docs/phase-03/implementation-notes.md` and screenshot/test evidence are present; Codex Reviewer ACCEPT required before Phase 04.

## 10. Codex Reviewer handoff

Reviewer should verify: (1) Phase 01/02 regressions, (2) separation of UI and mocked services, (3) every active control is functional within demo, (4) coherent cross-screen state and IDs, (5) accessible keyboard/UI states, (6) URL fixture import is local-only, (7) no fabricated real download speed/file output, (8) test evidence and Phase 04 backend service-replacement map.

If rejected, Antigravity Fixer modifies only issues listed in review and sends changes back to Codex Reviewer. Do not push, merge, release or begin Phase 04 without explicit authorization and an ACCEPT outcome.

## 11. Ready-to-paste Antigravity prompt

> Đọc toàn bộ `PHASE_03_MANGA_DOWNLOADER_1989.md`, `AGENTS.md`, `docs/ARCHITECTURE.md` và tài liệu bàn giao Phase 01–02. Kiểm tra source code hiện có trước khi sửa; không khởi tạo lại project. Triển khai lần lượt P03-01 → P03-09, hoàn thiện tất cả màn hình Home, Discover/URL Import, Manga Detail, Library/Favorites, Downloads, Sample Reader, Settings/Sources theo Dark Purple Design System sẵn có. Dùng mock services, dữ liệu fixture nhất quán và nhãn Demo rõ ràng; tuyệt đối không giả vờ rằng tải truyện, tìm kiếm Internet, lưu file hoặc đọc CBZ thực tế đã hoạt động. Tất cả nút đang bật phải có hành vi UI thực; không cấp thêm quyền Tauri rộng. Sau từng task chạy kiểm tra phù hợp và sửa lỗi. Kết thúc bằng test, lint, typecheck, build, Rust regression, Windows smoke tests khi môi trường cho phép; tạo `docs/phase-03/implementation-notes.md` gồm file thay đổi, kết quả lệnh/test thực tế, screenshots, hạn chế và mapping thay mock services bằng backend Phase 04–07. Chỉ bàn giao cho Codex Reviewer; không tự push, merge hoặc deploy.
