# Kế hoạch cải thiện UI — "Reader-first Polish"

> **Trạng thái:** ✅ ĐÃ TRIỂN KHAI C1→C5 (commit UI polish này) — kết quả + lệch so với plan ghi ở §7 cuối file.
> **Loại:** plan cải tiến UI thuần frontend (không phase engine mới)
> **Tác giả:** Codex Planner · **Ngày:** 2026-09-23
> **Baseline:** `master` @ `3f4eda2` (sau review toàn project 4 vòng; trạng thái `RC_TESTED — PUBLICATION BLOCKED`)
> **Thực thi bởi:** Antigravity Implementer · Review: Codex Reviewer (quy trình "review N vòng" của user)

---

## 1. Mục tiêu

Nâng chất lượng cảm nhận của app **không đổi hành vi chức năng**: reader thành trung tâm trải nghiệm, Home có phân tầng thị giác, Downloads "sống" realtime, lỗi nhất quán về feedback + a11y vi mô.

**Ràng buộc bất biến (không được phá — xem `docs/HANDOFF.md` §3):**
- Không thêm dependency state/UI mới (không TanStack/Zustand/UI-kit) — giữ stack `useAsyncData` + `useInvalidation` (6 domain).
- Không đổi CSP, capabilities, hợp đồng IPC đang freeze cho RC; **chỉ được dùng DTO field ĐÃ TỒN** phía frontend. Trường dữ liệu mới → việc riêng, không nằm trong plan này.
- Rust-authoritative: frontend không nhận/suy diễn path hay URL; ảnh trang đi qua `readerGetPageBytes` → blob như hiện tại.
- Demo/mock chỉ trong `import.meta.env.DEV`.
- `prefers-reduced-motion` phải tôn trọng (`tokens.css` đã có nền tảng).

**DTO field có sẵn dùng được ngay:** `DownloadTaskDto.{status, completedPages, totalPages, revision, errorMessage, nextRetryAt, finishedAt, outputRelpath}` + event `download-task-updated.{downloadedBytes, speedBytesPerSecond}` + `live` map trong `useDownloadQueue` (hiện mới dùng cho text, chưa visualize).

---

## 2. Kiến trúc liên quan (chỉ phần chạm tới)

| Vùng | File hiện tại | Vai trò |
|---|---|---|
| Design tokens | `src/styles/tokens.css`, `globals.css` | Dark Purple semantic tokens; nơi thêm token layer mới |
| Shell | `components/layout/{AppShell,WindowTitlebar,Sidebar,Topbar,PageContainer}.tsx` | Custom titlebar; sidebar 6 route |
| Home | `features/home/HomePage.tsx` + `components/{LocalContinueReading,StatCard}.tsx` | Stats + continue-reading từ IPC thật (`library_stats`, recent tasks) |
| Reader | `features/read/{ReadPage,ReaderModes}.tsx`, `usePageImages.ts`, `useReaderProgress.ts`, `readerLogic.ts` | 3 mode đơn/đôi/doc, LTR/RTL, zoom, fullscreen, blob LRU, debounce progress 800ms |
| Downloads | `features/downloads/{DownloadsPage.tsx,useDownloadQueue.ts}`, `components/ui/DownloadCard.tsx` | Queue mirror revision-guarded; `live` bytes/speed từ event |
| Library | `features/library/{LibraryPage,LocalMangaPage,FavoritesPage}.tsx`, `components/ui/MangaCard.tsx` | grid/list + viewMode localStorage |
| Feedback | `components/ui/{Dialog,Tooltip,StatusBadge,EmptyState,ErrorState,Progress}.tsx`, `app/AppEventBridge.tsx` | Dialog đã focus-trap; lỗi queue đang render `role="alert"` ở top page |

---

## 3. Workstreams — bước implement có thứ tự

### UI-1 · Reader immersive (priority cao nhất — màn dùng nhiều nhất)

1. **Auto-hide chrome**: trong `ReadPage.tsx`, toolbar + titlebar region fade (opacity/translate, transition 200ms) sau 2.5s không input khi đang đọc; mousemove/phím/scroll lại hiện. Trạng thái hide là ephemeral state trong `ReadPage`, KHÔNG vào `useReaderProgress`. `prefers-reduced-motion` → không fade, chỉ hiện/ẩn tức thì.
2. **Progress ring góc trang**: thay label số thuần bằng SVG ring nhỏ (component `ReaderProgressRing` mới trong `features/read/components/`), input `{page, pageCount}` — reuse `progressPercent` của `readerLogic.ts` (logic thuần, đã có test).
3. **Gutter double-page**: khi `mode==="double"`, kẻ separator mảnh giữa primary/secondary dùng token `--border-subtle`; RTL vẫn secondary|primary theo `spreadOrder` hiện có — chỉ CSS, không đụng logic.
4. **Chế độ đèn bàn (lamp theme)**: thêm data-attribute `data-reader-theme="default|sepia|night"` trên root reader, CSS vars local trong `globals.css` (background ấm/tối hơn nền mặc định). Bpersist qua `window.localStorage` cùng pattern `library.viewMode` (`readViewMode()` hiện có làm mẫu). Không đụng `tokens.css` toàn cục, không đụng settings blob Rust.
5. **Nút thoát fullscreen về Esc**: nếu đã fullscreen/CSS-fallback, Esc trả focus về control cuối (mô hình Dialog đã có focus-restore — trích cùng helper).

**Edge cases phải xử lý:** auto-hide không được ẩn khi dialog confirm đang mở hoặc user đang tương tác control (focus trong toolbar); lamp theme phải đọc an toàn khi `localStorage` throw (try/catch như `readViewMode`); zoom >100% + double mode giữ nguyên scroll container hiện tại (không refactor layout song song).

### UI-2 · Home có phân tầng

1. **Hero Continue Reading**: card lớn (cover placeholder + title + chapter + %ring mini) từ dữ liệu `LocalContinueReading` hiện có — chỉ đổi layout/component tree trong `HomePage.tsx`, không đổi fetch. Không có progress → hero biến "empty state mời đọc" (reuse `EmptyState`).
2. **Rail ngang "Tải gần đây"** (recent tasks đã fetch sẵn): `overflow-x-auto` + `scroll-snap`; mỗi item là `MangaCard` compact mới variant prop `density="compact"` (prop optional, default giữ hành vi cũ — call-sites khác không đổi).
3. **Skeleton theo layout thật**: thay spinner/placeholder chung bằng skeleton mô phỏng hero + rail (component `Skeleton` đã có — thêm prop `shape` nếu cần).

**Ràng buộc:** `LocalContinueReading`/`StatCard` là nguồn dữ liệu đã test ở phase 08 (tests/phase08.test.tsx) — mọi assert cũ phải còn PASS; test mới chỉ thêm, không sửa test cũ để "cho qua".

### UI-3 · Downloads dashboard sống

1. **Sparkline tốc độ** trong `DownloadCard`: nhận prop mới `speedHistory?: number[]` — `useDownloadQueue` giữ ring-buffer ≤30 mẫu/task từ event `speedBytesPerSecond` (state `live` mở rộng trong hook hiện có — không đổi contract ngoài hook). SVG inline ~40 line, không lib.
2. **ETA text** (`còn ~Xp`) tính frontend từ `totalPages - completedPages` và speed gần nhất; `speed null/0` → ẩn ETA (không hiển thị `∞` hay NaN).
3. **Group theo manga**: `DownloadsPage` gom task cùng `chapterId→manga`… **không có mangaId trong DTO** → group theo prefix `outputRelpath` không khả dụng cho task chưa xong → **quyết định của plan: group by `chapterId` là vô nghĩa; giữ flat list, thêm filter trạng thái (Đang/Chờ/Lỗi/Xong)** bằng query client-side trên `tasks` đã load (không IPC mới).
4. **Error inline per-card**: `applyAction` reject → gắn lỗi vào card tương ứng (map `actionErrors: Record<taskId,string>` trong hook) + tự clear khi task có event revision mới; bỏ dần alert top-page cho action lỗi (alert top-page chỉ giữ cho lỗi snapshot-list).
5. **Nút Pause ẩn đúng lifecycle**: `allowedDownloadActions` đã mirror (verifying/exporting chỉ cancel) — bảo đảm `DownloadCard` re-render theo event (đã revision-guarded).

### UI-4 · Feedback & a11y vi mô

1. **Toast system nhẹ**: `components/ui/Toast.tsx` mới (portal + timeout, stackable ≤3, `role="status"`), dùng cho: action thành công (queued, resumed), copied, clear cache xong. KHÔNG thay dialog confirm xóa (giữ Dialog đã review).
2. **Tooltip cho nút disabled**: `Tooltip.tsx` hiện không kích hoạt trên `disabled button` (không focus được) → bọc `span tabIndex={0}` khi `disabled` hoặc chuyển `aria-disabled` + click-noop — chọn phương án aria-disabled (không đổi hành vi).
3. **CommandPalette combobox chuẩn**: thêm `aria-activedescendant` nối input ↔ option id; focus-visible outline cho item đang highlight (đã có ↑↓/Enter, thiếu announce).
4. **`ErrorState` có nút Retry thật**: props `onRetry?` — wire vào các màn đang tự render nút retry nội bộ (`LibraryPage.retryNonce` pattern) để hợp nhất, không bắt buộc đủ mọi màn trong UI-4 (incremental).

### UI-5 · Cờ technical-debt (KHÔNG làm trong plan này — ghi để khỏi tái phạm)

- Không refactor `useAsyncData`→data-layer mới khi chưa có ≥3 màn cần cache shared.
- Không đổi reader sang protocol `asset:` (CSP đã gỡ allowance có chủ đích).
- Không tự-host font trong plan UI này (việc riêng, đụng CSP).
- Multi-select batch action trong Library → chỉ khi có IPC command thật (hiện queue per-chapter đã batch ở `queue_chapter_downloads` — frontend đủ dùng, chưa cấp thiết).

---

## 4. Validation (bắt buộc mỗi workstream)

Gates như hiện hành: `npm run typecheck · lint · test · build` + `cargo` không cần chạy nếu **zero Rust diff** (UI-1..4 thuần frontend; nếu có dòng Rust nào phình ra → giải trình trong report).
- Test mới (vitest, không snapshot CSS): reader theme persist + ring math thuần; `useDownloadQueue` ring-buffer + actionErrors clear-on-revision; `allowedDownloadActions` giữ nguyên; palette `aria-activedescendant` có trong DOM sau khi mở.
- Test cũ PHẢI giữ nguyên kết quả (đặc biệt `tests/phase07/phase08/reviewRound3`).
- `npm run tauri dev` smoke cuối kỳ: mở reader 1 chapter fixture, bật lamp + auto-hide, tải 1 task fixture thấy sparkline + ETA, Ctrl+K hoạt động.
- Review theo quy trình user: mỗi vòng đọc diff, BLOCKING sửa trong vòng đó.

## 5. Tiêu chí_accept

1. Reader: chrome tự ẩn/hiện đúng kịch bản 1/2/3 (đọc/interaction/dialog); 3 theme chuyển được và reload giữ theme.
2. Home: hero + rail hiển thị từ dữ liệu IPC thật (dev tools network tab chứng minh không mock trong prod path).
3. Downloads: task đang chạy hiện sparkline + ETA; lỗi action hiển thị tại card, tự clear khi revision mới về.
4. A11y: axe không thêm violation mới trên 4 màn chạm tới; keyboard-only walkthrough (Tab/↑↓/Enter/Esc) pass.
5. Không đổi: contract IPC, CSP, capabilities, dependency graph (package.json diff = 0).
6. Toàn bộ test cũ PASS + test mới PASS; commit local riêng cho plan này (push khi user yêu cầu).

## 6. Phân rã thực hiện đề xuất (thứ tự commit)

| Bước | Nội dung | Size ước lượng |
|---|---|---|
| C1 | UI-4.2/4.3 (tooltip-disabled, palette a11y) — rủi ro thấp nhất, mở màn | S |
| C2 | UI-1 reader immersive + lamp theme | M |
| C3 | UI-3 downloads sống + error inline | M |
| C4 | UI-2 home hierarchy | M |
| C5 | UI-4.1/4.4 toast + ErrorState retry + polish + docs reconcile (HANDOFF, README §UI) | S–M |

Mỗi C = 1 round "implement → review N vòng của user → gates → commit".

---

## 7. Nhật ký triển khai (C1→C5, 2026-09-23)

| Cung | Trạng thái | Ghi chú lệch plan (thành thật) |
|---|---|---|
| C1 a11y | ✅ | Tooltip thêm `focusableWhenDisabled` (wrapper tabindex=0 khi trigger disabled) tại 3 call-site; CommandPalette: input giờ `role=combobox` + `aria-expanded/controls/activedescendant`, option có `id`. RemoteDetailPage tooltip "Phase 06–07" stale → thay hint đúng hành vi hiện hành. Outline active đã có sẵn (`--focus-ring`). |
| C2 reader | ✅ | Auto-hide 2.5s + clear timer unmount + KHÔNG ẩn khi focus đang trong toolbar; pager dưới cũng ẩn; `ReaderProgressRing` mới (reuse `progressPercent`, aria-hidden vì đã có page indicator thật); 3 lamp theme persist `localStorage["reader.theme"]` (try/catch), scope bằng `data-reader-theme` + CSS var cục bộ shell (không đụng tokens toàn cục); gutter double = gap 6px + shadow; focus-return khi Esc thoát fullscreen. Không đổi logic `usePageImages`/`useReaderProgress`. |
| C3 downloads | ✅ | `useDownloadQueue`: `speedHistory` ring ≤30 mẫu/task (terminal tự xóa), `etaSeconds` từ page-progression thật, `actionErrors` per-task — lỗi action hiển thị inline tại card (role=alert), TỰ CLEAR khi revision mới về; banner top-page chỉ còn cho lỗi snapshot. Sparkline SVG trong `DownloadCard`. Filter trạng thái client-side ĐÃ TỒN từ trước → không thêm mới. Group-theo-manga bị chính plan bác (không có mangaId trong DTO) — giữ flat + filter, đúng fallback plan ghi. |
| C4 home | ✅ | Hero Continue Reading (bìa chữ cái đầu + progress bar + 2 CTA) + strip các mục sau; rail ngang `scroll-snap` cho tác vụ gần đây (badge + mini-bar + đếm trang); skeleton theo layout thật. Data logic giữ nguyên 100% (test cũ T03/T04/T08 PASS không sửa). |
| C5 feedback | ✅ | `toastStore.ts` (store tách file vì react-refresh) + `ToastViewport` mount 1 lần trong AppShell, `role=status`/`aria-live=polite`, max 3, tự tan 4s; wire: 4 action Downloads + queue thành công LocalMangaPage. `ErrorState` thêm `onRetry`/`retryLabel` (custom `action` vẫn thắng) — đã dùng ở DownloadsPage; chuyển hết các màn khác để phase sau (plan cho phép incremental). |
| Test mới | 7 case | `tests/uiPlan.test.tsx`: formatEtaSeconds guards, sparkline math, ring-buffer cap + terminal clear, action-error placement/self-clear, toast cap/lifetime/auto-dismiss. |
| Gates | ✅ | typecheck 0 · lint 0 warning · vitest **92/92** (85 cũ nguyên vẹn + 7 mới) · vite build OK · `tauri dev` smoke xem §HANDOFF. Zero diff Rust/IPC/CSP/capabilities/package.json — đúng ràng buộc §1. |

### 7.1 Review 3 vòng (2026-09-23, sau `f106cb0`)

| Vòng | Finding | Sửa |
|---|---|---|
| 1 | ETA hiển thị cả task paused/queued (giá trị cũ misleading) | `DownloadsPage`: ETA chỉ khi `resolving|downloading` |
| 1 | Tooltip `role=tooltip` không được screen reader đọc nếu thiếu association | `useId` + `aria-describedby` trên wrapper (khi `focusableWhenDisabled`) |
| 1 | Progress ring đè lên nút "Sau →" của pager | `bottom: calc(3.5rem + var(--space-2))` |
| 2 | **Sepia theme làm chrome không đọc được**: `.reader-tool` hardcode `--text-primary` (sáng trên nền kem), `.reader-toolbar__chapter` hardcode `--text-muted` | Chuyển sang `var(--reader-chrome-fg)` (+opacity cho chapter) — cả 3 theme đều tương phản đúng |
| 2 | Placeholder trang đang tải hardcode `#131022` lệch nền sepia | `rgba(128,128,128,.22)` trung tính |
| 3 | Đối chiếu claims ↔ diff: 7/7 test case đúng số khai, `f106cb0` zero-diff `src-tauri`+`package.json` đúng cam kết, test cũ không sửa assert | — |

Gates sau mỗi vòng 1/2: typecheck 0 · lint 0 · vitest 92/92 · build OK.
