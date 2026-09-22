# Phase 09 — Windows Smoke

Mọi mục có trạng thái thật. "AUTO" = đã có test tự động phía sau; "MANUAL" = cần người thao tác (chưa ai thao tác ⇒ NOT RUN, không phải PASS).

## Đã chạy tự động/agent trong Phase 09

| # | Hạng mục | Kết quả | Ghi chú |
|---|---|---|---|
| 1 | `npm run tauri dev` — app cửa sổ thật, WebView2, custom titlebar | **PASS** (nhiều lần, gần nhất P08 review 3 vòng) | 0 panic; tray + notification plugin load; DB schema 5 |
| 2 | `npm run tauri build` — release + bundle | **PASS EXIT=0** 2026-09-23 | MSI 6.46 MB + NSIS 4.47 MB; SHA256 trong `evidence/bundle-checksums-2026-09-23.txt`; gate treo từ Phase 04 chính thức khép ở mức build |
| 3 | Chạy **release exe** trực tiếp (`target/release/manga-downloader-1989.exe`) | **PASS** | sống 12 s, process healthy, đóng bằng terminate (force) — shutdown-path test bằng close thường ở mục 4 manual |
| 4 | Ctrl+K palette, dialogs, focus trap, Escape | **AUTO PASS** (vitest) | jsdom-level |
| 5 | Tồn tại dữ liệu qua restart app (DB cùng file) | **AUTO PASS** (Rust reopen-pool tests) | |

## Checklist MANUAL — TRẠNG THÁI: NOT RUN (cần user, ~20 phút)

Chạy `npm run tauri dev` (hoặc cài NSIS/MSI):

- [ ] Resize 1024×700 → fullscreen; không mất CTA; focus viền còn thấy (T09-49)
- [ ] Display scaling 125% / 150% / 200% — chữ/không vỡ, titlebar kéo được (T09-49)
- [ ] Multi-monitor: kéo sang màn ngoài, maximize đúng screen
- [ ] Bật `closeToTray` trong Settings → bấm X → app ẩn, download đang chạy KHÔNG chết; tray → Show/Quit (T09-50)
- [ ] Bật notifications, tải 1 chapter fixture/live → toast "Download hoàn tất" hiện; Settings tắt → không toast (T09-25/50)
- [ ] Fullscreen reader (F / nút) trên WebView2 thật + Esc thoát; click-X khi đang đọc (flush progress — kiểm tra Continue Reading đúng trang) (T09-51)
- [ ] Ngắt mạng vật lý khi đang tải → task failed/paused đúng, nối lại Retry chạy (T09-16)
- [ ] Đĩa gần đầy / thư mục `library` đặt quyền read-only → lỗi rõ, không completed giả (T09-23)
- [ ] Sleep máy 5'+wake khi queue đang chạy → trạng thái nhất quán (T09-50)
- [ ] Cài NSIS + MSI trên máy sạch (WebView2 runtime có sẵn Win11), chạy app từ shortcut, mở library thật (T09-03/R-2)
- [ ] Keyboard-only journey Home→(Ctrl+K)→Reader→Esc, Tab thấy focus ở mọi màn (T09-51)
- [ ] 20 chương xếp hàng cùng lúc trong khi mở Library — Task Manager: RAM không tăng trần (T09-47/48)

Kết quả từng mục sẽ được điền + screenshot/log vào `evidence/` khi user chạy; **không mục nào được đánh PASS hộ**.
