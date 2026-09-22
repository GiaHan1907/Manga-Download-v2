# Manga Downloader 1989

Local-first desktop app for downloading and reading manga offline. Built with Tauri 2, React, TypeScript, Vite, Rust, and SQLx/SQLite.

> **Phase 01 — Project Foundation & Architecture** ✅ + **Phase 02 — Design System & UI Shell**  
> Phase 02 adds the Dark Purple design system, custom Windows titlebar, AppShell (sidebar/topbar), typed routing with honest placeholders, and reusable UI primitives. No business features (URL import, search, download queue, library, offline reader) are implemented yet — those come in Phase 03+. See `docs/phase-02/implementation-notes.md`.

---

## Stack

- **Frontend:** React 19 + TypeScript + Vite 8 + React Router (HashRouter) + Lucide icons
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
| `npm run test` | ✅ Pass | 8/8 tests (Phase 01 IPC states + Phase 02 routing/sidebar/404/dialog/windowService) |
| `npm run build` | ✅ Pass | Vite 8.3.0, 1920 modules → `dist/` (298 kB JS, gzip 93.9 kB) |
| `cargo check` | ✅ Pass | 1 warning (dead_code: `Internal` variant, expected until business logic) |
| `cargo clippy` | ✅ Pass | Same 1 warning |
| `cargo test` | ✅ Pass | 2/2 DB tests (init + idempotency) |
| `npm run tauri dev` | ✅ Smoke pass | App compiled, launched with custom titlebar, exited cleanly (code 0). Human visual checks (drag/resize/scaling) still recommended — see `docs/phase-02/implementation-notes.md` §5 |
| `npm run tauri build` | ⏳ Not run | Slow (~10-20 min); run before release |

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
│   └── phase-02/
│       └── implementation-notes.md  # Phase 02 audit, UI contract, test results, open issues
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
│   │                            # MangaCard, ChapterRow, DownloadCard (presentation-only)
│   ├── features/
│   │   ├── system/
│   │   │   └── SystemStatus.tsx # IPC consumer: loading/error/success states
│   │   └── placeholders/
│   │       └── PlaceholderPage.tsx  # Honest "Coming in Phase 03" pages per route
│   ├── services/
│   │   └── windowService.ts     # Tauri window API wrapper (minimize/maximize/close/drag)
│   ├── shared/
│   │   └── api/
│   │       ├── contracts.ts     # AppInfo, AppHealth, AppError types
│   │       └── desktop.ts       # Sole IPC boundary: getAppInfo, getAppHealth, normalizeIpcError
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
│   │   └── icon.ico             # 16x16 placeholder (dark purple #101016)
│   ├── migrations/
│   │   └── 0001_init.sql        # Creates app_settings table only
│   └── src/
│       ├── main.rs              # Windows subsystem attr, calls lib::run()
│       ├── lib.rs               # Builder: setup (DB init), invoke_handler (2 commands)
│       ├── app_state.rs         # AppState { db: SqlitePool }
│       ├── error.rs             # AppError enum → serialized { code, message, retryable }
│       ├── commands/
│       │   ├── mod.rs
│       │   └── system.rs        # app_get_info, app_health_check
│       └── db/
│           └── mod.rs           # init_pool: create_dir_all, SqlitePool, embedded migration
└── tests/
    ├── setup.ts                 # Testing Library cleanup; per-file mocks
    ├── app.test.tsx             # IPC states + routing/sidebar/404/dialog keyboard tests
    └── windowService.test.ts    # Window command wrapping + rejection fallback
```

---

## Database Location

SQLite database is stored in the Tauri app data directory:

- **Windows:** `%APPDATA%\com.manga1989.downloader\manga-downloader-1989.db`
- Resolved at runtime via `app.path().app_data_dir()` (Tauri path API)
- Created automatically on first run; migrations applied via `sqlx::migrate!("./migrations")`
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
- Reusable UI primitives (Button, Input, Dialog, Tabs, Tooltip, Progress, StatusBadge, Skeleton, EmptyState, ErrorState) and presentation-only domain cards (MangaCard, ChapterRow, DownloadCard) — fixture data only, clearly labeled
- ErrorBoundary + component/route/window-service tests

**NOT implemented (Phase 03+):**
- URL import from manga sources
- In-app search (the topbar search affordance only navigates to the Discover placeholder)
- Download queue (chapters, images, retry, concurrency, real progress events)
- CBZ packaging
- Library management (no library DB tables)
- Offline reader
- Manga/chapter/source/download/library DB tables (only `app_settings` exists)
- Settings persistence (placeholder UI does not write `app_settings`)
- Tailwind, shadcn/ui, Radix, Motion, Zustand, TanStack Query
- Tauri plugins: filesystem, shell, http, dialog, notification, updater
- Installer signing, auto-update, CI/CD

**No fake UI:** Placeholder pages state "Coming in Phase 03" / "Không có dữ liệu thật"; fixture cards are labeled as fixtures; no control pretends backend functionality exists.

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
- **CSP:** `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost`. No wildcards, no remote scripts. Tauri auto-injects nonces for its own scripts.
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
