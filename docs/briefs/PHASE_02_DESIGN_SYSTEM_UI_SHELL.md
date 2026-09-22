# MANGA DOWNLOADER 1989 — PHASE 02 IMPLEMENTATION PLAN

**Phase:** 02 — Design System & UI Shell  
**Platform:** Windows 10/11  
**Stack:** React + TypeScript + Vite + Tauri 2 + Rust + SQLite  
**Theme:** Dark Purple  
**Implementer:** Antigravity  
**Prerequisite:** Phase 01 has passed review; preserve its architecture and existing working code.

## 1. Objective and boundaries

Build a polished, keyboard-accessible, responsive desktop shell and reusable UI system. The result must run in the existing Tauri Windows app. Phase 03 will build full Home, Discover, Manga Detail, Downloads, Library, Reader and Settings screens on top of this foundation.

**In scope:** design tokens; fonts and icons; window chrome/titlebar; sidebar/topbar/content layout; routing and route placeholders; reusable controls; keyboard navigation; empty/loading/error primitives; motion and reduced-motion behavior; component documentation; UI tests.

**Out of scope:** manga search/import, source adapters, downloading, reader engine, complete feature screens, real download progress, production backend endpoints, authentication, and new database features. Do not fake functional buttons or fabricate backend data.

## 2. Architectural constraints

1. Inspect the actual Phase 01 repository before editing. Reuse existing package manager, router, state management, IPC wrappers, lint/test tools and naming conventions; do not recreate the project or overwrite working config.
2. React components are presentational where practical; business logic belongs in feature modules. Keep desktop-window commands in one typed `windowService` abstraction.
3. Prefer Tailwind CSS and existing shadcn/ui/Radix components where already installed; add only necessary dependencies. Keep theme tokens in one authoritative location.
4. Never grant broad shell/filesystem permissions for UI work. Tauri capabilities must be limited to required window commands. Preserve native close/minimize/maximize, resizing, dragging, keyboard behavior and Windows accessibility.
5. The custom titlebar must work with Windows scaling and multi-monitor configurations. Use Tauri-supported window drag regions or documented window APIs; interactive titlebar controls must not accidentally initiate dragging.
6. Route placeholders must clearly indicate future functionality; navigation must work without pretending that backend features exist.

## 3. Design specification

| Token | Value |
|---|---|
| Background | `#101016` |
| Sidebar | `#171720` |
| Surface | `#20202C` |
| Elevated | `#292938` |
| Primary | `#A78BFA` |
| Primary hover | `#8B5CF6` |
| Text primary | `#F5F3FF` |
| Text muted | `#A1A1B5` |
| Success | `#34D399` |
| Warning | `#FBBF24` |
| Error | `#F87171` |

Use semantic variables (`--background`, `--surface`, `--primary`, etc.) and accessible foreground pairs rather than hard-coding palette values throughout components. Main font: Inter with robust system fallbacks; support Vietnamese diacritics and long Japanese manga titles. Use Lucide icons. Avoid excessive blur, neon, glow, and decorative microcopy.

**Desktop layout:** default 1440×900, minimum 1024×700 (unless Phase 01 config establishes a justified alternative); sidebar 240px expanded, approximately 72px collapsed; titlebar approximately 48px; content padding 24–32px; spacing scale 4/8/12/16/24/32; card radius 12px, dialog radius 16px. Support vertical scrolling, Windows 100/125/150/200% display scaling, and text zoom. No clipped controls at the minimum size. Animation generally 150–250ms and disabled/reduced under `prefers-reduced-motion`.

## 4. Target project structure (adapt to existing repository)

```text
src/
  app/
    App.tsx
    router.tsx
  components/
    layout/
      AppShell.tsx
      WindowTitlebar.tsx
      Sidebar.tsx
      Topbar.tsx
      PageContainer.tsx
    ui/
      Button.tsx
      Input.tsx
      Dialog.tsx
      Tabs.tsx
      Tooltip.tsx
      Progress.tsx
      StatusBadge.tsx
      Skeleton.tsx
      EmptyState.tsx
      ErrorState.tsx
      MangaCard.tsx
      ChapterRow.tsx
      DownloadCard.tsx
  features/
    placeholders/
      PlaceholderPage.tsx
  services/
    windowService.ts
  styles/
    tokens.css
    globals.css
  tests/
    ...
```

Do not create duplicate implementations of already existing files or UI components. Names above are suggested logical locations, not a mandate to break Phase 01 conventions.

## 5. Implementation tasks

### P02-01 — Repository audit and UI contract
- Review Phase 01 implementation, build/test commands, current routes, Tauri window configuration, capabilities and existing components.
- Record the baseline build/test results and any constraints in `docs/phase-02/implementation-notes.md`.
- Define component props, route identifiers, layout breakpoints, theme token conventions and keyboard interaction rules.
- **Acceptance:** no destructive reinitialization; all proposed changes fit the existing project.

### P02-02 — Tokens, fonts and global styles
- Add semantic theme variables, typography scale, spacing, radii, focus rings, selection colors and scrollbar treatment.
- Implement Dark Purple as default; ensure contrast and visible keyboard focus. Add a reduced-motion path.
- **Acceptance:** all sample components consume semantic tokens; Vietnamese text displays correctly; no global style regression.

### P02-03 — Windows titlebar and window actions
- Implement application icon/name, draggable title region, minimize, maximize/restore, close, and double-click-to-maximize where supported.
- Encapsulate Tauri window calls in `windowService`; handle rejected commands without crashing the app.
- Preserve resize borders and accessible names/keyboard focus on controls; prevent drag behavior on clickable elements.
- **Acceptance:** each window control works in packaged/dev Windows builds; window can be dragged, resized and restored on multiple display scales.

### P02-04 — Sidebar, topbar and AppShell
- Implement 240px sidebar with active item, icon + label, collapse/expand, keyboard navigation and tooltip in collapsed state.
- Navigation: Home, Discover, Library, Downloads, Favorites, Settings. Reader and Manga Detail can be non-sidebar routes.
- Add Topbar with page title, a visual search affordance that routes to Discover, and a settings shortcut; do not imply live search exists.
- Implement scrollable main content, persistent shell, page transitions that respect reduced motion, and minimum-size behavior.
- **Acceptance:** all navigation targets resolve, active state is correct, no content obscured by titlebar/sidebar, no dead-end navigation.

### P02-05 — Reusable UI primitives
- Implement or adapt Button, Input, Dialog, Tabs, Tooltip, Progress, StatusBadge, Skeleton, EmptyState, ErrorState.
- Add domain presentation primitives MangaCard (2:3 cover and fallback), ChapterRow, DownloadCard. Use clearly labeled fixture data only in component previews/tests.
- Cover loading, disabled, focus, hover, empty, long-title and error states. Use proper button semantics and accessible labels.
- **Acceptance:** components are typed, reusable, keyboard-operable and have visual states; no backend calls inside presentation components.

### P02-06 — Route placeholders and UX polish
- Add honest placeholders for Home, Discover, Manga Detail, Library, Downloads, Reader, Favorites and Settings as appropriate to existing router.
- Provide a 404 route, ErrorBoundary, loading shell and safe empty states. Only show controls that have real navigation or explicitly state `Coming in Phase 03`.
- **Acceptance:** direct navigation/reload does not crash; placeholder pages identify future functionality without simulated success.

### P02-07 — Tests, documentation and handoff
- Add component tests for sidebar active/collapse state, window-service behavior with mocks, dialog keyboard behavior, route rendering and error states.
- Run lint, typecheck, frontend tests, Rust checks and Tauri Windows dev/build smoke tests; capture exact commands and outputs.
- Record screenshots at default/minimum window size and Windows 100/150% scaling if available. Document limitations if test environment is not Windows.
- **Acceptance:** no unaddressed high-severity issues; reviewer can reproduce all checks.

## 6. Testing matrix

| Test | Expected result |
|---|---|
| Start app | AppShell appears with Dark Purple theme, no fatal errors |
| Window controls | Minimize, maximize/restore and close work |
| Drag/resize | Titlebar dragging and window resize work; buttons do not drag |
| Sidebar | Active route highlights; collapsed labels available via tooltip |
| Routing | Every route resolves; unknown route shows 404 |
| Keyboard | Tab order, Enter/Space, Escape and visible focus work |
| Display scaling | No clipped controls at 100/125/150/200% where tested |
| Small window | Main content scrolls; titlebar and navigation remain usable |
| Reduced motion | Nonessential transitions are suppressed |
| UI states | Loading/empty/error/disabled states render correctly |
| Regression | Phase 01 database initialization and IPC smoke checks still pass |

## 7. Definition of Done

- Phase 01 remains functional; no working configuration or source code is overwritten without justification.
- AppShell, titlebar, sidebar, topbar, route placeholders and reusable components are implemented and integrated.
- Design tokens are centralized and used consistently; layout works at the documented minimum size.
- Window actions are functional in a Windows Tauri build; no unnecessarily broad capabilities are introduced.
- Lint, TypeScript typecheck, Rust check and automated tests pass; Windows build/dev smoke test is recorded.
- `docs/phase-02/implementation-notes.md` contains changed files, commands, test results, screenshots/locations, known issues and Phase 03 handoff notes.
- Codex Reviewer approves the phase before release or Phase 03 begins.

## 8. Antigravity execution prompt

> Read `PHASE_02_MANGA_DOWNLOADER_1989.md` and inspect the existing Phase 01 repository first. Implement Phase 02 tasks P02-01 through P02-07 in order, preserving existing architecture and functionality. Build the actual Windows Tauri UI shell and Dark Purple design system; do not implement downloader/search/reader business logic or claim mock functionality is real. Reuse existing dependencies/components where possible. After each task, run relevant checks, fix failures and summarize changed files. At the end, run all available lint/typecheck/tests and a Windows Tauri smoke build, produce `docs/phase-02/implementation-notes.md`, and report acceptance criteria with evidence. If the environment cannot run Windows-specific tests, state exactly what remains unverified. Do not push, merge or deploy without explicit authorization; hand the results to Codex Reviewer.
