# Phase 03 Self-Review (theo brief PHASE_03_MANGA_DOWNLOADER_1989.md)

**Ngày review:** 2026-09-22 · **Hình thức:** tự review đối chiếu brief trên code hiện tại (commits `ba6696f`, `dd5434c`, cộng dồn review fixes các phase trước).

## 1. Bằng chứng đã kiểm tra (grep/git, không suy đoán)

| Yêu cầu brief | Kiểm chứng | Kết quả |
|---|---|---|
| §2.3 Screens không import fixture trực tiếp | grep `mock/|fixtures` trong `src/features/` = **0 kết quả**; mọi dữ liệu đi qua `services/provider.ts` | ✅ |
| §2.4 Không dùng event Rust giả progress | grep `listen(|emit(` = 0; progress chỉ từ `completedPages/totalPages` mock | ✅ |
| §2.5 ID ổn định, seed bất định | fixture ID cố định; grep `Math.random|Date.now|new Date(` = 0 (chỉ comment); timestamps ISO cứng | ✅ (xem F1 về seq task) |
| §2.6 URL import local-only | matcher regex `demo.manga1989.example` so text; grep `fetch(|XMLHttpRequest|WebSocket` = 0; 3 test rejects | ✅ |
| §2.7 Không dead control | mọi control bật có hành vi thật trong prototype (15 test integration); future-only đều `disabled` + tooltip/label lý do (Read sample, Thêm nguồn, output formats) | ✅ |
| §2.8 Bảo toàn Phase 01/02 + không quyền mới | `git log 0cbc28d..HEAD -- src-tauri` chỉ có commit fix Phase 01 review; commit Phase 03 **không chạm** `src-tauri/`; capability vẫn 5 window perms; health IPC thật qua `SystemStatus` | ✅ |
| §2.9 A11y | focus ring, aria-labels, role=status announcements, dialog focus trap + restore (`dd5434c`), sidebar/tabs/reader keyboard, `isTypingTarget` guard | ✅ (sweep đầy đủ vẫn cần mắt người) |
| §2.10 Strict TS | grep `: any|as any|@ts-ignore` = 0; discriminated unions (`AsyncData`, `ImportFeedback`, `DownloadStatus` transitions); debounce có cleanup; không mock side-effect ở module init (chỉ seed data thuần) | ✅ |
| §4 Mock rules | ≥8 manga/2 nguồn, completed/failed/queued/paused seeds, reader chapter 4 trang; reset helper dev-only | ✅ |
| §7 Test matrix | T03-01→13 map vào `tests/phase03.test.tsx` + `mockServices.test.ts`; 47/47 pass; gates lint/typecheck/build/cargo pass | ✅ |
| §8 Commands | dùng đúng scripts sẵn có; kết quả thật ghi ở notes §6, không gate nào báo pass giả | ✅ |

## 2. Findings

| # | Finding | Phân loại | Xử lý |
|---|---|---|---|
| F1 | `taskCounter` là biến module ngoài `MockState`, `resetMockState()` không reset → task id sinh ra **không tái lập** giữa các lần reset, trái rule "predictable mock reset" (§4) | BLOCKING | ✅ Đã chuyển seq vào `MockState.nextTaskSeq` (id pad `demo-task-001`), reset khôi phục; thêm test hồi quy `generates deterministic task ids reproducible after reset`; 47/47 pass |
| F2 | Reader ghi progress page 0 ngay khi mount trước khi restore | ADVISORY | Không sửa: `getProgress` đọc state đồng bộ tại thời điểm gọi (trước write đầu tiên), restore đúng trang; hành vi được kiểm chứng bằng test bounds/restore |
| F3 | §3.4 "action menus" triển khai dạng hàng nút thay vì context menu | ADVISORY | Nút bàn-phím-được đầy đủ, không thêm abstraction; ghi chú nếu reviewer UI muốn menu kiểu Chuột-phải |
| F4 | Prefetch trang kề (§3.6 "if useful") | ADVISORY | Bỏ có chủ đích — prototype 3–4 trang SVG local |
| F5 | Screenshot 1440×900/1024×700 + scales chưa chụp | ~~thiếu~~ | Đã ghi `not captured` trung thực trong notes §5/§9; cần desktop session của user |

## 3. Definition of Done §9 — đánh giá lại

- [x] P03-01→09 theo thứ tự, deviations ghi chú giải (state library tự viết thay Zustand/React Query — không thêm dependency không cần, phù hợp AGENTS "no unnecessary dependencies")
- [x] 9 route thật, điều hướng hoạt động, không dead primary action
- [x] Theme/tokens/window shell Phase 02 tái sử dụng; min 1024×700
- [x] Mock tách sau typed API; fixture coherent + nhãn Demo
- [x] invalid URL / missing records / empty / transition errors xử lý (tests)
- [x] Phase 01 health + Phase 02 shell còn hoạt động (cargo 2/2, dev smoke exit 0 ×2, build pass)
- [x] Commands có môi trường chạy được đều chạy và ghi output thật
- [x] `docs/phase-03/implementation-notes.md` + docs/README/screen-checklist

**Kết luận: ACCEPT sau khi sửa F1.** Blocker duy nhất là determinism của task id; các mục còn lại là advisory/manual đã khai báo.

## 4. Vòng review 2 (fresh eyes, cùng ngày — soi acceptance §3 bằng hành vi, không chỉ state)

| # | Finding | Phân loại | Xử lý |
|---|---|---|---|
| F6 | `SearchPanel` của Discover gọi `search(query, selectedSources)` với rỗng = "không lọc" → **manga của nguồn đã tắt vẫn hiện trong Discover**, vi phạm acceptance §3.7 "toggling source affects Discover". Test vòng 1 chỉ kiểm `listSources()` đổi state, không kiểm kết quả thật — bài học: test phải assertion trên hành vi end-to-end | BLOCKING | ✅ `DiscoverPage.tsx`: effective filter = selection ∩ enabled, mặc định là danh sách enabled; tất cả nguồn tắt → 0 kết quả. Test hồi quy `disabling a source removes its manga from Discover results (§3.7)`. Gates: 48/48, lint/typecheck pass |

**Kết luận vòng 2: ACCEPT** — không còn BLOCKING mở. Các mục F2–F5 giữ nguyên phân loại advisory/manual.
