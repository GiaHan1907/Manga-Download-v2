# Manga Downloader 1989

Local-first desktop app for downloading and reading manga offline. Built with Tauri 2, React, TypeScript, Vite, Rust, and SQLx/SQLite.

> **Bắt đầu từ đây (đặc biệt cho AI/dev tiếp theo):** đọc [`docs/HANDOFF.md`](docs/HANDOFF.md) — trạng thái, bất biến kiến trúc, việc còn tồn. Brief và plan các phase trong `docs/briefs/`, `docs/plans/`; self-review từng phase trong `docs/phase-*/`.

> **Phase 01 — Foundation** ✅ · **Phase 02 — Design System & UI Shell** ✅ · **Phase 03 — Core UI Screens (Interactive Prototype)** ✅ · **Phase 04 — Database & Backend Core** ✅  
> Phase 04: migration `0002_domain.sql` + Rust repositories/services + 10 typed IPC commands. Settings/Library/Favorites now use **real SQLite** (initially empty until Phase 05); Discover/Detail/Downloads/Reader remain an explicitly labeled demo sandbox. Contract: `docs/IPC_CONTRACT.md`, schema: `docs/phase-04/schema.md`.  
> Phase 03 added all product screens (Home, Discover/URL import, Manga Detail, Library, Favorites, Downloads, Sample Reader, Settings/Sources) on top of the Phase 02 shell, backed by **typed mock services with deterministic fixture data**. This is a prototype: no real search, downloading, CBZ export, or file storage. See `docs/phase-03/`.

---

## Stack

- **Frontend:** React 19 + TypeScript + Vite 8 + React Router (HashRouter) + Lucide icons
- **State/services (Phase 03):** typed service interfaces + deterministic mock provider (`src/services/`), single mock store (`useSyncExternalStore`) — swap to IPC backend in Phase 04–07 without reworking screens
- **Desktop/Backend:** Tauri 2 + Rust 1.98 (MSVC)
- **Database:** SQLite via SQLx 0.8 (embedded migrations)
- **Target:** Windows 10/11 x64

---

## Windows Prerequisites

Before you begin, ensure the following are installed:

| Tool | Version | Notes |
|---|---|---|
| **Node.js** | `>=20.19` (tested with v24.15.0) | Brief recommends LTS 20.19+ or 22.12+; v24 works fine with Vite 8 + React 19 |
| **npm** | Comes with Node | Used for all frontend tooling |
| **Rust** | stable MSVC (`rustup default stable-msvc`) | Verified with rustc 1.98.1 |
| **Visual Studio Build Tools** | "Desktop development with C++" workload + Windows SDK | Required for compiling Rust MSVC target |
| **Microsoft Edge WebView2 Runtime** | Evergreen | Most Win10/11 have it preinstalled; if missing, install from [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) |
| **Tauri CLI v2** | Installed locally as devDependency | No global install needed |

Verify your toolchain:

```powershell
node --version
npm --version
rustc --version
cargo --version
rustup show
```

Expected output should show `x86_64-pc-windows-msvc` as the active target.

---

## Install & Run

```powershell
# Clone repo (if not already)
cd D:\Project\Mangav2

# Install dependencies
npm ci

# Start dev mode (opens desktop window with Vite HMR)
npm run tauri dev
```

The first `tauri dev` run will compile the Rust backend (~3-5 minutes). Subsequent runs are faster due to caching.

---

## Quality Gates

Run these commands to verify the project is healthy:

```powershell
# Frontend
npm run lint          # ESLint flat config
npm run typecheck     # tsc -b
npm run test          # vitest run (jsdom + @testing-library/react)
npm run build         # tsc -b && vite build → dist/

# Backend (from src-tauri/)
cd src-tauri
cargo check           # Compile check
cargo clippy          # Rust linter
cargo test            # Unit tests (DB init + migration idempotency)
cd ..

# Full desktop build (slow, ~10-20 min, produces installer)
npm run tauri build
```

All gates should exit with code 0. Current status:

| Gate | Status | Notes |
|---|---|---|
| `npm ci` | ✅ Pass | 234 packages, 0 vulnerabilities |
| `npm run lint` | ✅ Pass | ESLint flat config, no errors |
| `npm run typecheck` | ✅ Pass | `tsc -b`, no type errors |
| `npm run test` | ✅ Pass | 85/85 tests (Phase 01 IPC → Phase 08 integration + reader logic + review-round regressions; counts updated at each self-review — current truth in `docs/HANDOFF.md` §1a) |
| `npm run build` | ✅ Pass | Vite → `dist/` (~386 kB JS; exact size in HANDOFF gate line) |
| `cargo check` | ✅ Pass | clean |
| `cargo clippy --all-targets -- -D warnings` | ✅ Pass | zero warnings |
| `cargo test` | ✅ Pass | 134/134 (engine state-machine + checkpoint/exporter adversarial + CBZ parser handcrafted-ZIP tests + migration upgrade tests + HTTP policy/SSRF + discovery + library/reader + storage/settings + perf tripwires) |
| `npm run tauri dev` | ✅ Smoke | Last smoke after whole-project review 4 vòng (2026-09-23): app chạy thật, DB reopen schema 5 không reset |
| `npm run tauri build` | ✅ Pass (2026-09-23, NSIS-only) | Exit 0; artifact + SHA-256 hiện hành trong `docs/phase-10/ARTIFACT_MANIFEST.md` (`2245f30e…a90674`). Chạy thử bản cài đặt trên VM sạch: chờ user xác nhận |

---

## Project Structure

```
manga-downloader-1989/
├── README.md
├── AGENTS.md                    # Multi-agent workflow rules (preserved from baseline)
├── .editorconfig
├── .gitignore
├── package.json
├── package-lock.json
├── tsconfig.json                # Solution-style, references app + node configs
├── tsconfig.app.json            # Frontend TS config (React, paths @/*)
├── tsconfig.node.json           # Vite/Vitest config TS
├── vite.config.ts               # Port 1420, strictPort, React plugin
├── vitest.config.ts             # jsdom, tests/**, setup file
├── eslint.config.js             # Flat config: typescript-eslint + react-hooks + react-refresh
├── index.html                   # Vite entry, mounts #root
├── docs/
│   ├── phase-02/
│   │   └── implementation-notes.md  # Phase 02 audit, UI contract, test results, open issues
│   └── phase-03/
│       ├── README.md                # Routes, UI states, sample-data disclaimer
│       ├── screen-checklist.md      # Per-screen acceptance checklist
│       └── implementation-notes.md  # Phase 03 audit, mock→backend swap map, results
├── src/
│   ├── main.tsx                 # React root, imports globals.css
│   ├── vite-env.d.ts
│   ├── app/
│   │   ├── App.tsx              # Mounts AppRouter
│   │   ├── router.tsx           # HashRouter, route map, 404
│   │   ├── routes.ts            # Typed route contract (single source of truth)
│   │   └── ErrorBoundary.tsx    # UI crash guard
│   ├── components/
│   │   ├── layout/              # AppShell, WindowTitlebar, Sidebar, Topbar, PageContainer
│   │   └── ui/                  # Button, Input, Dialog, Tabs, Tooltip, Progress,
│   │                            # StatusBadge, Skeleton, EmptyState, ErrorState,
│   │                            # MangaCard, DownloadCard (presentation-only)
│   ├── assets/
│   │   └── sample-reader/           # Original neutral SVG pages + cover placeholder (no 3rd-party art)
│   ├── types/
│   │   └── backend.ts                   # DTO mirrors of Rust payloads (Phase 04)
│   ├── domain/                      # Typed models
│   │   ├── manga.ts                 # MangaSummary/Detail, ChapterSummary, MangaStatus
│   │   ├── download.ts              # DemoDownloadTask + discriminated transitions
│   │   └── reader.ts                # ReaderPage, ReadingProgress, zoom/page clamps
│   ├── services/
│   │   ├── catalog.service.ts       # Interfaces only — screens never import fixtures
│   │   ├── library.service.ts
│   │   ├── download.service.ts
│   │   ├── reader.service.ts
│   │   ├── settings.service.ts
│   │   ├── settings.api.ts               # REAL IPC adapter (Phase 04)
│   │   ├── local-library.service.ts      # production-mode Local contract
│   │   ├── local-library.api.ts          # REAL IPC adapter (Phase 04)
│   │   ├── provider.ts              # Wires mock impls; Phase 04+ swaps to IPC here
│   │   ├── windowService.ts         # Tauri window API wrapper (minimize/maximize/close/drag)
│   │   └── mock/
│   │       ├── fixtures.ts          # Deterministic demo catalog, fixed IDs/ISO timestamps
│   │       ├── store.ts             # Single mutable mock state + subscribe + resetMockState
│   │       └── mockServices.ts      # Fixture-backed service implementations
│   ├── features/
│   │   ├── home/                    # HomePage + StatCard, LocalContinueReading (real), RecentActivity
│   │   ├── discover/                # DiscoverPage + UrlImportPanel + useDebouncedValue
│   │   ├── manga-detail/            # MangaDetailPage + ChapterSelectRow
│   │   ├── library/                 # LibraryPage, FavoritesPage
│   │   ├── downloads/               # DownloadsPage (mock transitions)
│   │   ├── reader/                  # ReaderPage + ReaderImage (SVG sample pages)
│   │   ├── settings/                # SettingsPage, SourcesPage
│   │   └── system/
│   │       └── SystemStatus.tsx     # IPC consumer: loading/error/success states
│   ├── shared/
│   │   ├── api/
│   │   │   ├── contracts.ts         # AppInfo, AppHealth, AppError types
│   │   │   └── desktop.ts           # Sole IPC boundary: typed wrappers for all 46 commands + event listeners + binary page normalization
│   │   └── hooks/                   # useAsyncData, useServices (store), useInvalidation (6-domain bus), useDebouncedValue helpers
│   └── styles/
│       ├── tokens.css           # Semantic Dark Purple design tokens (colors, type, spacing, radii)
│       └── globals.css          # Reset + shell/UI component styles, reduced-motion
├── src-tauri/
│   ├── Cargo.toml               # tauri 2, sqlx 0.8, serde, thiserror
│   ├── Cargo.lock
│   ├── build.rs                 # tauri_build::build()
│   ├── tauri.conf.json          # identifier, CSP, devUrl, frontendDist, window config
│   ├── capabilities/
│   │   └── default.json         # main window: core:default + 5 explicit window permissions
│   ├── icons/
│   │   └── icon.ico (+ png set) # Real app icon — generated by scripts/make-icon.mjs, expanded via `npx tauri icon` (Phase 10)
│   ├── migrations/
│   │   └── 0001_init.sql … 0006_watch.sql   # Forward-only chain; 0004 executed as runtime-repaired SQL (src/db/migrations_repaired/, original checksum kept)
│   └── src/
│       ├── main.rs              # Windows subsystem attr, calls lib::run()
│       ├── lib.rs               # Builder: setup (DB init + storage prep + recovery pass + tray/notifications + F1 watcher loop), 46 commands
│       ├── app_state.rs         # AppState { db, registry, coordinator, events, … }
│       ├── error.rs             # AppError enum → serialized { code, message, retryable }
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── system.rs        # app_get_info, app_health_check (+schemaVersion)
│       │   ├── settings.rs      # get/update/reset — persisted in app_settings
│       │   ├── local_catalog.rs # list/get local manga (bounded pagination)
│       │   ├── favorites.rs     # set/list persisted favorites
│       │   ├── reading_progress.rs # get/save with chapter-ownership check
│       │   ├── discovery.rs     # source registry CRUD + search/detail/import-URL (Phase 05)
│       │   ├── downloads.rs     # queue/pause/resume/cancel/retry/priority + paged list (Phase 06)
│       │   ├── library.rs       # library list/stats/chapters/reconcile/delete (Phase 07)
│       │   ├── reader.rs        # chapter manifest + per-page bytes via tauri::ipc::Response (Phase 07)
│       │   └── tasks.rs         # read-only task metadata
│       ├── domain/              # typed DTOs + settings/ids validation
│       ├── sources/             # adapter trait + mangadex/madara + GuardedHttp (exact-host, DNS gate, pacing)
│       ├── downloader/          # engine: scheduler/worker/transfer/checkpoint/exporter/storage/recovery (Phase 06)
│       ├── reader/              # CBZ (STORE-only, bounded central-dir parse) + image folders + sniff (Phase 07)
│       ├── services/            # business invariants + storage/settings/library/reader services
│       └── db/
│           ├── mod.rs           # init_pool + production_migrator (0004 runtime repair, checksum preserved)
│           ├── migrations_repaired/ # repaired 0004 SQL (see docs/HANDOFF.md §1a)
│           └── repositories/    # parameterized SQLx repos, BEGIN IMMEDIATE write tx
└── tests/
    ├── setup.ts                 # Testing Library cleanup; per-file mocks
    ├── app.test.tsx             # IPC states + routing/sidebar/404/dialog keyboard tests
    ├── windowService.test.ts    # Window command wrapping + rejection fallback
    ├── mockServices.test.ts     # Fixture integrity, search, queue dedupe, transitions, reset
    ├── readerLogic.test.ts      # Pure reader layout logic
    ├── phase03..08.test.tsx     # Per-phase integration matrices (T03-01.., T05 live-UI, T06 queue UI, T07 reader/library, T08 favorites/history/palette/settings)
    └── reviewRound3.test.tsx    # Whole-project review round-3 regressions (queue monotonic merge, blob lifecycle, action table)
```

---

## Database Location

SQLite database is stored in the Tauri app data directory:

- **Windows:** `%APPDATA%\com.manga1989.downloader\manga-downloader-1989.db`
- Resolved at runtime via `app.path().app_data_dir()` (Tauri path API)
- Created automatically on first run; migrations applied via `db::production_migrator()` — the `sqlx::migrate!("./migrations")` set with migration 0004's SQL swapped for the checksum-preserving repaired variant (see `docs/HANDOFF.md` §1a, `src-tauri/src/db/mod.rs`)
- **No machine-specific paths are committed to the repo**

To verify DB location during dev, add a temporary `println!("{:?}", app_data_dir);` in `src-tauri/src/lib.rs` setup hook (dev builds only).

---

## Phase Scope Limits

**Implemented in Phase 1:**
- Tauri 2 desktop window (dark purple shell)
- Two typed IPC commands: `app_get_info`, `app_health_check`
- SQLite app-data DB with embedded migration (`app_settings` table only)
- Strict capabilities and CSP (no wildcards/remote scripts)
- Lint, typecheck, frontend test, Rust test, Vite build, cargo check/clippy/test

**Implemented in Phase 2:**
- Dark Purple design system: semantic tokens, typography/spacing/radii, focus rings, reduced-motion
- Custom Windows titlebar (drag region, double-click maximize, minimize/maximize/close via `windowService`) with 5 explicit core window permissions
- AppShell: collapsible sidebar with active-route highlight + tooltips, topbar, scrollable content
- Typed HashRouter routing with honest placeholder pages (Home/Discover/Library/Downloads/Favorites/Settings/Manga Detail/Reader) and 404
- Reusable UI primitives (Button, Input, Dialog, Tabs, Tooltip, Progress, StatusBadge, Skeleton, EmptyState, ErrorState) and presentation-only domain cards (MangaCard, ChapterRow, DownloadCard) — fixture data only, clearly labeled *(historical: ChapterRow was later removed as dead code in the whole-project 4-round review; the remaining primitives are live)*
- ErrorBoundary + component/route/window-service tests

**Implemented in Phase 3 (interactive prototype — mock data only):**
- Full screen set: Home dashboard, Discover search + demo URL import, Manga Detail chapter selection/queue, Library + Favorites, Downloads queue with mock transitions, Sample Reader (single/double/vertical, LTR/RTL, zoom), Settings + Sources
- Typed domain models + service interfaces (`src/services/*.service.ts`) with deterministic fixture-backed mock implementations behind a provider (`src/services/provider.ts`)
- Cross-screen shared mock state (favorites, queue, reading progress) via a single external store; session-scoped by design
- Original neutral SVG sample pages in `src/assets/sample-reader/` (no third-party copyrighted images)
- Persistent "Demo · dữ liệu mẫu" badge; all future-only controls visibly disabled with reasons

**NOT implemented (Phase 04+):**
- Real source adapters / arbitrary URL fetching / in-app search over the Internet (the demo matcher only compares strings against a fictitious `demo.manga1989.example` fixture)
- Download queue (chapters, images, retry, concurrency, real progress events)
- CBZ packaging
- Library management (no library DB tables)
- Offline reader
- Manga/chapter/source/download/library DB tables (only `app_settings` exists)
- Settings persistence (placeholder UI does not write `app_settings`)
- Tailwind, shadcn/ui, Radix, Motion, Zustand, TanStack Query
- Tauri plugins: filesystem, shell, http, dialog, notification, updater
- Installer signing, auto-update, CI/CD

**No fake UI:** Phase 02 placeholders were replaced by Phase 03 screens that only manipulate clearly-labeled mock state; future-only actions (Read without sample pages, Add source, output formats) are visibly disabled with an explanation, and nothing pretends a real download, file write or export happened.

---

## IPC Architecture

All frontend→backend communication goes through a single typed wrapper:

```
Component → src/shared/api/desktop.ts → invoke("command_name") → Rust command
```

- `desktop.ts` is the **only** file importing `invoke` from `@tauri-apps/api/core`
- Contracts defined in `src/shared/api/contracts.ts` (`AppInfo`, `AppHealth`, `AppError`)
- Rust structs use `#[serde(rename_all = "camelCase")]` to match TS types
- Errors are normalized to `{ code, message, retryable }` — no raw SQL, paths, or stack traces sent to UI

Registered commands (exactly two in Phase 1):
- `app_get_info` → returns `{ name, version, platform }`
- `app_health_check` → returns `{ status: "ok", database: "ready" }` (only after DB pool init succeeds)

---

## Security

- **Capabilities:** `src-tauri/capabilities/default.json` grants `core:default` plus exactly five explicit core window permissions (`allow-minimize`, `allow-toggle-maximize`, `allow-close`, `allow-is-maximized`, `allow-start-dragging`) to window `main` only — needed for the custom titlebar. No `shell:default`, `fs:default`, `http:default`, no plugins, no remote origins.
- **CSP:** `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob:`. No wildcards, no remote scripts, no `asset:` protocol (reader pages cross IPC as bytes → blob URLs). Tauri auto-injects nonces for its own scripts.
- **DB access:** Only Rust touches SQLite. Frontend sends data via validated command contracts.
- **No `withGlobalTauri`:** Tauri API scoped to `@tauri-apps/api` imports.

---

## Handoff Notes (Phase 1 → Phase 2)

### What was done
- Scaffolded Tauri 2 + React 19 + TypeScript + Vite 8 + Rust/SQLx project from scratch (manual file authoring, no interactive scaffolders)
- Established git baseline preserving `AGENTS.md` (multi-agent workflow rules)
- Implemented typed IPC boundary with two system commands
- Configured SQLite app-data DB with embedded migration (`app_settings` table)
- Applied strict Tauri capabilities and CSP
- Created frontend smoke test (vitest + jsdom + invoke mocks) and Rust DB test (tempfile + tokio)
- All headless quality gates pass

### Open items for Phase 2
- **Icon:** `src-tauri/icons/icon.ico` is a 16x16 placeholder (dark purple square). Replace with proper app icon before release.
- **`AppError::Internal` variant:** Currently unused (dead_code warning). Will be needed when business logic errors arise.
- **Manual verification:** `npm run tauri dev` process starts successfully; visual verification (window opens, dark purple bg, IPC data displays, DB created at `%APPDATA%`) requires manual check.
- **`npm run tauri build`:** Not run during Phase 1 (slow, optional). Verify before release.
- **WAL mode:** Not enabled (brief §7: avoid hard-to-reverse storage config in foundation phase). Revisit if concurrent read/write performance becomes an issue.
- **AppManifest command allowlist:** Skipped (optional, API support unconfirmed on locked Tauri version). Commands are registered explicitly in `generate_handler!`.

### Assumptions
- Node v24.15.0 works with Vite 8 + React 19 (brief recommended 20.19+/22.12+ LTS, but v24 is compatible)
- SQLx 0.8.6 is stable and compatible with Tauri 2.11.6
- `core:default` capability permission is sufficient for Phase 1 shell (no custom core permissions enumerated)
- Runtime SQLx queries (no `query!` macros) — avoids `DATABASE_URL`/offline cache complexity

### Confirmation
**No URL import, search, download, library, or offline reader features were implemented in Phase 1.** Only the foundation scaffold exists.

---

## License

TBD — not specified in Phase 1 brief.

---

## References

- [Tauri v2 Docs](https://v2.tauri.app/)
- [Vite Docs](https://vite.dev/)
- [SQLx SQLite](https://docs.rs/sqlx/latest/sqlx/sqlite/)
- [React 19](https://react.dev/)
- Brief: `PHASE_01_IMPLEMENTATION_PLAN.md` (provided by user)
