# Phase 08 — Self-review (3 vòng, theo chỉ đạo user sau commit `1da98b0`)

Phân loại BLOCKING (sửa ngay trong vòng) / ADVISORY (ghi nhận). Mọi vòng chạy lại toàn bộ gates.

## Vòng 1 — backend Windows/storage/settings

**BLOCKING đã sửa:**

- **R1-F1 — `build_tray(app.handle())?` làm fail CẢ startup** nếu Windows không cho tạo tray (thiếu icon, group policy). Trái policy tự áp của phase ("platform feature không được brick app" — recovery/cleanup đều log-and-continue). Sửa: `if let Err(error) = build_tray(...) { eprintln!(...) }` — tray mất thì app vẫn chạy, notification/settings khác vẫn hoạt động.
- **R1-F2 — Thiếu test Rust cho 2 settings key mới.** Thêm `t08_windows_flags_merge_and_validate` (patch camelCase → merge → validate).

**Xác minh không phải bug:**

- `failed` trong engine chỉ xảy ra khi HẾT retry budget (`record_failure` vs `requeue_with_backoff` — worker.rs:121-134) → notification không spam mỗi lần retry tạm thời.
- Startup cleanup chạy TRƯỚC `Coordinator::start()` → không race với worker; task `queued` (retry hẹn giờ) nằm trong active-set nên staging được GIỮ.
- `verify đã verify`: tray Quit → `app.exit(0)` → cùng `ExitRequested` shutdown path của X thường.

**ADVISORY:** `root_available` dùng `read_dir.is_ok() || exists()` (vế sau thừa nhưng vô hại — giữ).

## Vòng 2 — frontend bridge/palette/screens

**BLOCKING đã sửa:**

- **R2-F1 — Event flood:** `domainsForTaskStatus` bump `downloads` cho MỌI trạng thái trung gian; byte-progress frames phát ~mỗi 250ms/task → mọi màn hình lắng nghe domain (LocalMangaPage…) refetch nguyên danh sách liên tục khi đang tải. Sửa: chỉ terminal transitions mới bump (completed → downloads+library+home; failed/cancelled → downloads+home); intermediate → `[]` (DownloadsPage vốn tự merge frame live bằng revision-guard). Test cập nhật: `downloading/queued/verifying → []`.
- **R2-F2 — `window.confirm` trong embedded WebView** cho clear-cache: hành vi dialog không được bảo đảm trong mọi webview và lệch pattern app (mọi destructive action khác dùng inline confirm 2 bước). Sửa: inline confirm trong `StoragePanel` + note mô tả đúng thứ bị xóa; test assert bước trung gian không gọi IPC.
- **R2-F3 — Palette Escape chỉ hoạt động khi focus nằm trong dialog** (bấm chuột ra ngoài là mất đường đóng bằng Esc — plan §P08-03 yêu cầu Esc đóng). Sửa: Escape + Tab-trap ở window-level khi open; Tab bị chặn và trả focus về input.
- **R2-F4 — FavoritesPage nuốt lỗi unfavorite** (`throw error` → unhandled, im lặng). Sửa: hiển thị `actionError` role=alert.
- **R2-F5 — DownloadsPage empty-state sai ngữ cảnh:** filter không khớp trên trang có task vẫn báo "Chưa có tác vụ tải" (gây hiểu là library trống). Sửa: phân biệt "Không có tác vụ khớp bộ lọc" + nút Xem tất cả.

## Vòng 3 — adversarial

**BLOCKING đã sửa:**

- **R3-F1 — Settings dirty-state kẹt sau save:** `initial` không bao giờ刷新 → lưu xong nút vẫn "Lưu thay đổi" và mỗi lần đổi tab khác quay lại là stale. Sửa: `SettingsPage` refetch theo domain `settings` (save/reset đã bump) → dirty tính lại đúng.
- **R3-F2 — `RuntimeFlags::default()` đặt notify=false** trong khi `AppSettings::default()` = true → nếu startup load settings fail (DB lỗi nhẹ), notifications im lặng tắt, tài liệu nói bật. Sửa: `impl Default for RuntimeFlags` mirror đúng `AppSettings::default()` + test `t08_runtime_flags_mirror_settings_defaults` giữ hai bên khớp nhau.

**Đã kiểm, đạt:** bridge cleanup (disposed + unlisten list); palette không gọi `search_manga` (test); clear-cache path chặn tên task shape lạ; progress-fallback SQL deterministic (PK manga_id); cargo `--all-targets` clippy `-D warnings` 0 lỗi; `tauri dev` smoke vòng 3 chạy ~60s 0 error, port 1420 sạch sau đóng.

## Gates sau 3 vòng

typecheck/lint/build PASS · vitest **81/81** (10 files) · fmt/clippy PASS · cargo **106/106** (+2 test R1/R3 so với baseline 104) · `tauri dev` smoke PASS.

## Kết luận

7 BLOCKING (R1: 2, R2: 5… gồm 1 tính cả 2 vế) + 2 BLOCKING vòng 3 → tổng **9 BLOCKING đã sửa**, 1 advisory giữ có lý do. **ACCEPT sau 3 vòng review**; các mục NOT RUN manual/Windows thao tác thật vẫn nguyên như `test-results.md` khai.
