# Phase 02 — Design System & UI Shell: Implementation Notes

**Ngày:** 2026-09-22
**Người triển khai:** Qoder agent (theo brief `PHASE_02_MANGA_DOWNLOADER_1989.md`)
**Nền tảng:** Windows 11 x64, Node v24, Rust MSVC stable

## 1. P02-01 — Repository audit (kết quả)

Trạng thái Phase 01 trước khi chỉnh:

- Cấu trúc: `src/app` (App.tsx shell tối giản), `src/features/system` (SystemStatus), `src/shared/api` (contracts.ts + desktop.ts wrapper duy nhất), `src/styles` (tokens.css 8 màu + globals.css reset), `src-tauri` (Tauri 2 + SQLx/SQLite, 2 commands `app_get_info`/`app_health_check`, migration `0001_init.sql` chỉ tạo bảng `app_settings`).
- Chưa có: router, icon library, UI framework, custom titlebar. Window dùng decorations hệ thống, 1100×720 (min 800×600).
- Capability: `core:default` cho window `main`. CSP đã bật, không wildcard.
- Baseline gates (chạy trước khi chỉnh sửa): lint pass, typecheck pass, vitest 2/2 pass, vite build pass, `cargo check` pass (1 warning `Internal` chưa dùng — chủ đích từ Phase 01), `cargo test` 2/2 pass.
- `AGENTS.md` của user không bị đụng tới ở mọi bước.

Ràng buộc rút ra:

1. Giữ nguyên IPC boundary `src/shared/api/desktop.ts`; mọi `invoke()` mới (nếu có) phải qua wrapper. Phase 02 dùng `@tauri-apps/api/window` qua `src/services/windowService.ts` — đây là window API, không phải app command, nên đi qua service tách riêng đúng theo brief §2.2.
2. Không thêm backend command mới; không đổi schema DB.
3. Capability chỉ mở rộng bằng các permission window core cụ thể, không fs/shell/http.

## 2. UI contract

- **Routes** (`src/app/routes.ts`, một nguồn sự thật): home `/`, discover `/discover`, library `/library`, downloads `/downloads`, favorites `/favorites`, settings `/settings`, manga-detail `/manga/demo`, reader `/reader/demo`. Sidebar gồm 6 route đầu; manga-detail và reader là route ẩn khỏi sidebar. Unknown path → 404 trong shell.
- **Router:** `HashRouter` — reload/trực tiếp trong Tauri production không phụ thuộc server rewrite.
- **Tokens:** biến semantic (`--background`, `--surface`, `--primary`, `--text-primary`, `--success|warning|error`, spacing 4/8/12/16/24/32, radius 12/16, titlebar 48px, sidebar 240/72px). Alias `--color-*` cũ được giữ ánh xạ tới biến mới để tương thích.
- **Keyboard:** focus-visible ring dùng `--focus-ring`; sidebar nav hỗ trợ ArrowUp/ArrowDown; Tabs hỗ trợ ArrowLeft/Right; Dialog đóng bằng Escape; mọi control icon-only có `aria-label`.
- **Placeholder honesty:** mọi trang chưa có nghiệp vụ hiển thị rõ "Coming in Phase 03" và badge "Không có dữ liệu thật"; fixture chỉ nằm trong component preview/test.

## 3. File đã tạo / chỉnh sửa

### Tạo mới

| File | Vai trò |
|---|---|
| `src/app/routes.ts` | Route contract typed + metadata |
| `src/app/router.tsx` | HashRouter, route map, 404 page |
| `src/app/ErrorBoundary.tsx` | Chặn crash UI |
| `src/services/windowService.ts` | Bao bọc minimize/toggleMaximize/close/isMaximized, không crash khi từ chối |
| `src/components/layout/WindowTitlebar.tsx` | Titlebar tùy chỉnh, vùng kéo, double-click maximize |
| `src/components/layout/AppShell.tsx` | Titlebar + sidebar + topbar + scrollable content |
| `src/components/layout/Sidebar.tsx` | Nav active state, collapse, tooltip khi thu gọn |
| `src/components/layout/Topbar.tsx` | Tiêu đề trang + search affordance → Discover + settings shortcut |
| `src/components/layout/PageContainer.tsx` | Main content region |
| `src/components/ui/Button.tsx` | Button/link typed, variants |
| `src/components/ui/Input.tsx` | Labelled input |
| `src/components/ui/Dialog.tsx` | Modal, Escape, aria-modal |
| `src/components/ui/Tabs.tsx` | Tablist bàn phím |
| `src/components/ui/Tooltip.tsx` | Hover/focus tooltip |
| `src/components/ui/Progress.tsx` | progressbar ARIA |
| `src/components/ui/StatusBadge.tsx` | Badge trạng thái |
| `src/components/ui/Skeleton.tsx` | Loading skeleton |
| `src/components/ui/EmptyState.tsx` | Empty primitive |
| `src/components/ui/ErrorState.tsx` | Error primitive |
| `src/components/ui/MangaCard.tsx` | Domain card 2:3 + fallback (fixture) |
| `src/components/ui/ChapterRow.tsx` | Domain row (fixture) |
| `src/components/ui/DownloadCard.tsx` | Domain card (fixture) |
| `src/features/placeholders/PlaceholderPage.tsx` | Trang placeholder trung thực theo route |
| `tests/windowService.test.ts` | Test window service + fallback lỗi |
| `docs/phase-02/implementation-notes.md` | Tài liệu này |

### Chỉnh sửa

| File | Thay đổi |
|---|---|
| `src/styles/tokens.css` | 11 màu brief + semantic/typography/spacing/radius/focus tokens, alias tương thích, reduced-motion |
| `src/styles/globals.css` | Toàn bộ style shell/titlebar/sidebar/topbar/UI primitives, scrollbar, selection, focus ring, responsive breakpoint, reduced-motion |
| `src/app/App.tsx` | Shell tối giản → AppRouter |
| `src/features/system/SystemStatus.tsx` | Inline styles → panel/token, giữ nguyên IPC + message test (`Đang kiểm tra hệ thống...`, `Lỗi: <CODE>`) |
| `src-tauri/tauri.conf.json` | Window 1440×900 min 1024×700, `decorations: false`, `bundle.icon` trỏ icon hiện có |
| `src-tauri/capabilities/default.json` | Thêm 5 permission window core cụ thể (minimize, toggle-maximize, close, is-maximized, start-dragging) |
| `tests/app.test.tsx` | Thêm test route placeholder/active state, collapse sidebar, 404, dialog Escape |
| `tests/setup.ts` | Testing Library cleanup toàn cục |
| `vitest.config.ts` | include thêm `*.test.ts` |
| `package.json` / `package-lock.json` | Thêm `react-router-dom`, `lucide-react` |

Không đổi: Rust commands/db/error, migration, `AGENTS.md`, CSP, identifier.

## 4. Kết quả kiểm thử

| Gate | Lệnh | Kết quả |
|---|---|---|
| Lint | `npm run lint` | pass |
| Typecheck | `npm run typecheck` | pass |
| Frontend tests | `npm run test` | pass — 8/8 (2 file) |
| Frontend build | `npm run build` | pass — 1920 modules, 298 kB JS (gzip 93.9 kB) |
| Rust check | `cargo check` | pass (warning `Internal` unused — tồn tại có chủ đích từ Phase 01) |
| Rust lint | `cargo clippy` | pass (cùng 1 warning) |
| Rust tests | `cargo test` | pass — 2/2 DB tests |
| Tauri dev smoke | `npm run tauri dev` | Ran successfully: compiled, window launched with custom titlebar (`decorations: false`) and extended capabilities, exited code 0 — no permission denials or runtime errors in console output. Visual items below still need user confirmation. |
| Tauri release build | `npm run tauri build` | not run — xem mục 5 |

## 5. Kiểm tra thủ công Windows (test matrix)

Các mục cần mắt người trên desktop session được đánh dấu trạng thái; agent ghi trung thực:

| Test | Trạng thái |
|---|---|
| AppShell tối màu Dark Purple khi khởi động | chờ user xác nhận (dev smoke: mục 5.1) |
| Minimize / maximize-restore / close từ titlebar | chờ user xác nhận |
| Kéo cửa sổ ở vùng titlebar; nút không gây kéo; double-click maximize | chờ user xác nhận |
| Resize border còn hoạt động với `decorations: false` | chờ user xác nhận |
| Sidebar active highlight + tooltip khi thu gọn | automated test pass; visual chờ user |
| Routing: mọi route resolve, unknown → 404 | automated test pass |
| Bàn phím: Tab order, Enter/Space, Escape, focus visible | Dialog Escape automated; còn lại chờ user |
| Scale 100/125/150/200% | chưa kiểm tra — cần desktop session |
| Cửa sổ tối thiểu 1024×700 không clip control | chưa kiểm tra — cần desktop session |
| Reduced motion | CSS path đã implement; chưa screen-record xác nhận |
| Regression Phase 01: IPC + DB init | automated invoke mock pass; dev smoke exit 0 (app ran to clean close, DB reused from Phase 01) |

Screenshot: chưa chụp được trong môi trường headless; khi `tauri dev` chạy, user có thể chụp 2 kích thước default/minimum để đính kèm review.

## 6. Vấn đề còn tồn tại / ghi chú cho Phase 03

1. **Icon mới là placeholder 16×16** (từ Phase 01). Trước khi đóng gói phát hành cần icon PNG/ICO đa kích thước thật (`tauri icon`).
2. **`tauri build` release chưa chạy** — để tiết kiệm thời gian review cycle; cần chạy trước release Phase 02.
3. **`--font-sans` khai báo Inter nhưng chưa bundle font** — fallback system-ui/Segoe UI hoạt động, tiếng Việt/ Nhật hiển thị đúng qua font hệ thống; nếu muốn đúng Inter cần self-host (CSP `'self'` cấm CDN remote font).
4. **Custom titlebar + `decorations: false`**: snapping (Aero snap) qua drag region có thể cần xử lý thêm trên một số bản Windows; nếu thiếu, cân nhắc `data-tauri-drag-region` đã hỗ trợ maximize-by-double-click nhưng snap-edge cần kiểm thử thủ công.
5. `AppError::Internal` (Rust) vẫn unused — sẽ dùng khi có nghiệp vụ.
6. Settings placeholder chưa ghi `app_settings` — command đọc/ghi setting thuộc Phase 03.
7. Route manga-detail/reader dùng slug demo `/manga/demo`, `/reader/demo` — Phase 03 thay bằng param thật.
