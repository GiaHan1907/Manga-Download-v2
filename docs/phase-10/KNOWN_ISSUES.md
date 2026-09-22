# Phase 10 — Known Issues (ship-time, v1.0.0 RC)

Không có Critical/High mở (theo P09 SECURITY/RISK + review vòng). Medium/Low còn tồn — khai báo trong release notes/runbook, không ẩn:

| # | Mục | Severity | Tác động user | Kế hoạch |
|---|---|---|---|---|
| K-1 | Installer **unsigned** → SmartScreen prompt có thể xảy ra | Medium (distribution) | Thêm 1 bước xác nhận khi cài | Owner quyết định cert khi publish công khai (`SECURITY_SIGNING_UPDATER.md`) |
| K-2 | Không auto-update; upgrade thủ công cài đè | Medium | Tải installer mới khi muốn lên bản mới | Updater gated branch B — cần hạ tầng key/endpoint |
| K-3 | Live download journey + 13 mục manual Windows chưa chạy trên máy sạch | **High (verification gap, không phải defect)** | Có thể lộ lỗi môi trường thật | `WINDOWS_INSTALL_UPGRADE_TEST.md` checklist — user/VM |
| K-4 | CBZ deflate (archive nguồn ngoài) → `CBZ_INVALID` | Low | Không mở được file .cbz từ app khác | Chỉ làm khi có nhu cầu thật |
| K-5 | Library root cố định trong app-data (không đổi được) | Low | Không di chuyển thư viện ra ổ khác qua UI | Design có chủ đích P08; cân nhắc phase sau |
| K-6 | `reader_get_page_bytes` re-parse tail CBZ mỗi trang (đo 0.12 ms/trang — trong ngưỡng) | Low | Không cảm nhận được | Handle-cache nếu profile cho thấy nóng |
| K-7 | Checkpoint fsync-per-page ~4 ms (crash-safe đổi lấy tốc) | Low | Chương lớn chậm hơn vài giây | Không đổi semantics |
| K-8 | `prefers-reduced-motion` chưa honour (toolbar auto-hide/palette) | Low | Accessibility minor | Phase bảo trì |
| K-9 | 8 crate transitive "unmaintained" warnings (cargo audit) | Low | Chuỗi build tương lai | Tri-age khi pin release; không bump hàng loạt |
| K-10 | Search palette dùng `library_list` (LIKE) — library cực lớn có thể cần endpoint riêng | Low | Chưa đo ra vấn đề (4.9 ms @10k) | Đo trước, tối ưu sau |
| K-11 | ARM64 Windows chưa build/test | Low (scope) | Không có installer ARM | Work item riêng nếu cần |
| K-12 | Download đang chạy khi Quit từ tray → worker cancel cooperative ≤5s; task active về queued lần chạy sau (recovery) — **không** hứa "chạy nền sau khi thoát process" | Low (đúng thiết kế, cần hiểu đúng) | Người dùng có thể kỳ vọng sai | Đã ghi trong policy Settings/Windows |

**Deferred hợp lệ từ plan P10:** MSI (chỉ khi deployment yêu cầu), offline WebView2 installer (chỉ khi yêu cầu install offline-first), Microsoft Store, code signing, updater production.
