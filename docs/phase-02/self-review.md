# Phase 02 Self-Review (theo brief PHASE_02_MANGA_DOWNLOADER_1989.md)

**Ngày review:** 2026-09-22 · **Hình thức:** tự review đối chiếu brief, trên trạng thái code hiện tại (sau Phase 03 + các fix `dd5434c`, `feb1e5d`).

## 1. Bằng chứng đã kiểm tra

| Yêu cầu brief | Kiểm chứng | Kết quả |
|---|---|---|
| §2.1 Không re-scaffold, reuse Phase 01 | Notes §1 ghi baseline gates; Phase 01 không bị phá (regression cargo test 2/2 hiện tại) | ✅ |
| §2.2 Window commands trong một `windowService` duy nhất | grep `@tauri-apps/api/window` trong `src/` = **chỉ** `services/windowService.ts`; handle rejected command (return false + log, không crash) + test | ✅ |
| §2.3 Chỉ thêm dependency cần thiết | Runtime deps: react, react-dom, @tauri-apps/api + đúng 2 mục mới dùng thật (`react-router-dom`, `lucide-react`); không Tailwind/Radix/Zustand | ✅ |
| §2.4 Capability hẹp, không fs/shell/http | `git log -- capabilities` = chỉ 2 commit (01, 02); bản hiện tại là `core:default` + 5 window permission tường trùng, chỉ window `main` | ✅ |
| §2.5 Titlebar tương thích scaling; control không gây drag | `data-tauri-drag-region` chỉ trên vùng trống/text; 3 nút nằm ngoài drag region; resize giữ `resizable: true` | ✅ (chờ xác nhận mắt người ở mục 3) |
| §2.6 Placeholder trung thực | Đã đạt tại Phase 02; nay được thay bằng màn hình Phase 03 hợp lệ | ✅ |
| §3 Tokens | Cả 11 màu đúng hex brief; spacing 4/8/12/16/24/32; card radius 12px (`--radius-md`), dialog 16px (`--radius-lg`); titlebar 48px; sidebar 240/72px; transition 150–220ms + `prefers-reduced-motion` | ✅ |
| §3 Font Inter + fallback tiếng Việt/Nhật | `--font-sans: Inter, ui-sans-serif, system-ui, ...` — declared với fallback hệ thống | ⚠ xem F2 |
| §5 P02-01…07 | Notes + tokens + titlebar + shell + 13 primitive/card typed + route placeholders + tests | ✅ (tests: sidebar active/collapse, windowService mock, dialog Escape, route render, error states) |
| §6 Test matrix (automatable phần) | Vitest 46/44 pass; Rust regression pass; `tauri dev` smoke exit 0 ×2 lần | ✅ |

## 2. Findings

| # | Finding | Phân loại | Xử lý |
|---|---|---|---|
| F1 | DoD "Window actions are functional in **packaged**/dev Windows builds" — dev ✅, nhưng `npm run tauri build` (packaged) chưa chạy | BLOCKING (mở, đã khai báo `not run`) | Không fix được bằng code — là gate môi trường. **Bắt buộc chạy trước khi đóng Phase 02 về mặt phát hành**; đề xuất chạy ngay bây giờ hoặc chốt lịch trước release |
| F2 | Inter chỉ được *khai báo*, chưa bundle/self-host; máy không có Inter sẽ rơi về system-ui (vẫn hiển thị đúng tiếng Việt/Nhật) | ADVISORY | Giữ — brief cho phép "robust system fallbacks"; ghi chú upgrade self-host nếu cần đồng nhất font |
| F3 | Double-click maximize: handler JS trên drag region; nếu Tauri cũng xử lý native, hai lần toggle có thể triệt tiêu nhau | ADVISORY | Cần test thủ công 1 lần trên desktop (hành vi: double-click vùng titlebar trống) |
| F4 | Screenshot 1440×900 / 1024×700 / scale 125–200% chưa chụp (notes §5 đã ghi `not captured`) | ADVISORY | Cần desktop session; checklist từng mục đã liệt kê |
| F5 | Modal focus management (§2.9-style) chưa có lúc review Phase 02 gốc — đã được bổ sung ở self-review Phase 03 (`dd5434c`: focus trap + restore) | Đã xử lý | ✅ Retrospective ghi nhận |

## 3. Kết luận

**ACCEPT với 1 gate mở (F1):** toàn bộ yêu cầu code/config của Phase 02 đạt bằng chứng; duy nhất `npm run tauri build` (packaged smoke) chưa chạy và đã được khai báo trung thực là `not run` ngay từ handoff gốc. F3/F4 thuộc nhóm xác nhận mắt người trên desktop.
