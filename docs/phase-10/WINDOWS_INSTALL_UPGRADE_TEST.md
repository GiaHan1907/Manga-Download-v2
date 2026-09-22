# Phase 10 — Windows Install / Upgrade / Uninstall Test Guide

File này là **hướng dẫn + biểu mẫu kết quả** cho kiểm thử cài đặt thật trên Windows sạch. Agent KHÔNG chạy installer trên máy user (plan §2.4: chỉ test trên profile/VM sạch) ⇒ mọi mục chưa người nào thao tác = **NOT RUN** trong `RELEASE_TEST_MATRIX.md`.

## Chuẩn bị

- VM hoặc Windows user profile mới (Settings → Accounts → Other users) — không dùng profile chính có dữ liệu.
- Artifact: `Manga Downloader 1989_1.0.0_x64-setup.exe` — SHA-256 `2245f30e625ed5ecbe6c73ad63da4f56182b922d0b31ed5c7def702c22a90674`, 4,490,082 bytes (khớp `ARTIFACT_MANIFEST.md` bản rebuild sau review 4 vòng; hash cũ `6090b532…` đã superseded — đừng đối chiếu bản đó).
- Hai kịch bản WebView2: (A) runtime có sẵn (Win11 mặc định), (B) máy chưa có runtime — kiểm bằng `Get-AppxPackage *WebView2*` / registry `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`.

## Checklist (điền PASS/FAIL + ghi chú/ảnh vào cột Kết quả)

| # | Bước | Kỳ vọng | Kết quả |
|---|---|---|---|
| 1 | Chạy installer (user thường, không admin) | Hoàn tất per-user, shortcut Start Menu/Desktop, không UAC bất ngờ | ☐ |
| 2 | SmartScreen prompt (nếu hiện) | Hiện cảnh báo unknown publisher — **đúng tài liệu** (unsigned); xác nhận vẫn chạy được từ nguồn tin cậy | ☐ |
| 3 | Khởi động từ Start Menu | Cửa sổ chính hiện, icon đúng (mới, không placeholder), title "Manga Downloader 1989" | ☐ |
| 4 | About panel trong app | Bản ghi `1.0.0` | ☐ |
| 5 | (Kịch bản B) Máy thiếu WebView2 | Installer bootstrap tải runtime, app chạy; ghi lại hành vi mạng | ☐ |
| 6 | Offline: bật Library/Reader với CBZ có sẵn (copy fixture vào `%APPDATA%\com.manga1989.downloader\library` + DB fixture) | Đọc không cần mạng | ☐ |
| 7 | Live: thêm nguồn Madara được phép → tìm → lưu → queue 1 chương → CBZ verify | Hoàn tất thật, file trong `library/`, badge offline | ☐ |
| 8 | Tắt app giữa download → mở lại | Recovery về queued/paused trung thực, không completed giả | ☐ |
| 9 | Upgrade: cài bản 1.0.0 đè bản cũ đã có dữ liệu | Favorites/settings/progress/library còn nguyên; DB migration forward-only OK | ☐ |
| 10 | Uninstall (Settings → Apps) | Binary + shortcut bị gỡ; **%APPDATA%\com.manga1989.downloader còn nguyên** (policy giữ dữ liệu) | ☐ |
| 11 | Cài lại | Nhận diện dữ liệu cũ, không destructive migration | ☐ |
| 12 | Installer file hỏng (cắt đuôi byte) | NSIS báo lỗi, không để lại half-install, không đụng dữ liệu | ☐ |
| 13 | Đĩa gần đầy / thư mục app-data read-only | Lỗi rõ ràng, app không crash loop | ☐ |
| 14 | Scaling 125/150/200%, resize 1024×700 | Không mất CTA (nghiệm thu P09 T08-29 còn mở) | ☐ |
| 15 | Tray + close-to-tray + notification thật (bật trong Settings) | Đúng policy §settings-and-storage-policy P08 | ☐ |

## Ghi log môi trường kèm theo

OS build (`winver`), kiến trúc, WebView2 status, installer hash đã đối chiếu, screenshot các bước fail. Lưu vào `docs/phase-10/evidence/` (đã sanitize — không chứa đường dẫn cá nhân ngoài `%APPDATA%\com.manga1989...`).
