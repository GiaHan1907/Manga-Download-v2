# Phase 09 — Risk Register

Severity theo cổng release Phase 09 (§7 plan). "Trạng thái" phản ánh đúng việc đã làm trong phase này.

| # | Risk | Severity | Likelihood | Impact | Trạng thái / mitigation | Owner |
|---|---|---|---|---|---|---|
| R-1 | Live download journey chưa từng chạy trên nguồn thật (T06-24b/T08-36) — lỗi network-shape thật (TLS, cookie, CDN chậm) có thể lộ ra | **High** | Trung | UX/reporting, không mất dữ liệu (mọi đường fail đã test ở fixture) | **ĐÓNG 2026-09-23 (user-chỉ đạo, headless probe `#[ignore] live_r1`)**: Ninja Loạn Thị Tập 1 từ truyentuoitho.com — search→save→queue→worker thật→verify→export **188/188 trang, CBZ 49.578.260 B (≈47,3 MiB), attempts=0**; lộ 2 gap thật đã sửa (chapter-list AJAX `ajax/chapters/`, ảnh CDN khác host → allowlist imageHosts tường minh). Còn lại: xác nhận lại bằng thao tác trong app UI (checklist manual) | Done (headless) |
| R-2 | `tauri build` giờ EXIT=0 nhưng installer chưa được cài+chạy thật (MSI/NSIS mới build-level verified; AUMID toast, tray icon trong bundle) | Medium | Thấp | Release gate | Cài + smoke 2 bundle theo `WINDOWS_SMOKE.md` — việc của user/Phase 10 | User |
| R-3 | Checkpoint fsync-per-page: 4 ms/write → chapter 300 trang ≈ 1.2 s chỉ riêng durability; ổ chậm/AV có thể gấp 3–5 lần | Low–Medium | Cao trên HDD | Tốc độ download, không sai dữ liệu | Chấp nhận có chủ đích (crash-safe là contract P06); đo lại khi có HDD baseline | Dev |
| R-4 | Chưa có test tự động 20-task đồng thời end-to-end (RAM/CPU/event-rate) — mới có DB-contention half | Medium | Trung | Hiệu năng queue lớn | Kịch bản thủ công trong `WINDOWS_SMOKE.md` §4; tự động hoá cần local HTTPS fixture server — defer Phase 10 | Dev |
| R-5 | `reader_get_page_bytes` mở+parse tail CBZ mỗi request; trang thứ N phải parse lại central | Low | Cao (mọi lần lật) | Latency đọc (đo được 18.9 ms/300-trang — nằm trong ngưỡng) | Chấp nhận + ghi handoff P07; nếu đo thấy nóng thì thêm handle cache | Dev |
| R-6 | prefers-reduced-motion chưa honour ở toolbar auto-hide/palette transition | Low | — | Accessibility | Issue mở — cosmetic; không chặn release | Dev |
| R-7 | CBZ deflate từ nguồn ngoài vẫn `CBZ_INVALID` by contract | Low (known) | — | Mở archive ngoài | Đã document; chỉ làm khi có nhu cầu | PM |
| R-8 | `cargo audit` warning: 8 crate unmaintained transitive (fxhash, proc-macro-error…) | Low | — | Chuỗi build tương lai | Không major-bump dây chuyền; sẽ xem khi pin release Phase 10 | Dev |
| R-9 | Path-Display của `io::Error` từng có thể vào stderr (S-2) | Low | — | Info-leak tối thiểu (chỉ dev console) | **Đã sửa** — log chỉ `kind()` | Done |
| R-10 | cbz.rs adversarial coverage từng = 0 (S-1) | Medium (test gap) | — | Regression an toàn file | **Đã sửa** — 7 test builder thủ công | Done |
| R-11 | Testkit overflow khi fixture >255 trang (S-3) | Low | — | Chỉ test | **Đã sửa** | Done |

**Kết luận:** Không còn **Critical/High chưa xử lý** ở mức code/testable. Hai mục High/Medium phụ thuộc người thật (R-1 live, R-2 installer) là **BLOCKED hợp lệ** theo quy ước — không được tính là PASS.
