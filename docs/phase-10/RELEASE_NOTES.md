# Release Notes — Manga Downloader 1989 v1.0.0 (Windows x64)

**Trạng thái phát hành:** RELEASE CANDIDATE — **UNSIGNED**, chưa publish (xem `PHASE_10_HANDOFF.md`).

## App làm được gì (tất cả đã kiểm chứng tự động — nguồn: docs/phase-09)

- **Discover & Import:** tìm kiếm đa nguồn thật + import URL qua engine Rust với allowlist host nghiêm ngặt (HTTPS-only, chặn SSRF/loopback/metadata, pace ≥3s/host, giới hạn kích thước). Provider tích hợp sẵn: MangaDex; nguồn Madara/WordPress do người dùng đăng ký.
- **Tải chương:** hàng đợi có giới hạn (chapter × ảnh, backoff/retry có ngân sách, pause/resume/cancel/retry), checkpoint chống mất trang khi crash, xuất **CBZ đã kiểm chứng** hoặc thư mục ảnh; lịch sử + phân trang). File lưu theo tên sạch: `library/<tên-truyện-không-dấu>/chapter-<số>.cbz` (VD `ninja-loan-thi/chapter-1.cbz`); các file tải trước đó giữ nguyên đường dẫn cũ đã lưu trong DB). Có **tải nhanh bằng dán link chương** ở tab Downloads (link do bạn nhập; truyện chưa lưu sẽ tự lưu đúng luồng Discover) và nút **mở nơi lưu** (Explorer chọn đúng file/thư mục ảnh của chương đã tải; nút "Mở thư mục tải" mở gốc thư viện). Ở trang chi tiết truyện có nút **tải nhanh 5 / 10 / toàn bộ chương chưa tải** (theo thứ tự mục lục; tự chia lô 100 theo giới hạn hàng đợi).
- **Thư viện local & Favorites:** aggregate thật (số chương offline, dung lượng, tiến độ), sort/filter/tìm kiếm, reconcile không phá hủy, xóa disk-first an toàn trong app-root.
- **Offline Reader:** CBZ + image-folder, 3 chế độ (single/double/vertical-cuộn), LTR/RTL, zoom, fullscreen, điều hướng bàn phím, prefetch có thu hồi blob, resume đúng trang + vị trí cuộn gần đúng; **chạy hoàn toàn không cần mạng**.
- **Tiện ích:** Continue Reading thật, Command Palette Ctrl+K (local-only), notifications theo chương (tùy chọn), system tray + close-to-tray (tùy chọn), Settings validate phía Rust, thống kê dung lượng + dọn cache tái tạo được an toàn.

## Yêu cầu hệ thống

- Windows 10/11 x64. WebView2 Runtime (thường có sẵn trên Win 11; installer mặc định tải runtime nếu thiếu — **cần internet lần đầu**; xem `WINDOWS_INSTALL_UPGRADE_TEST.md`).
- ~50 MB chỗ cho app + dung lượng thư viện tùy người dùng (app-data, không cần admin — per-user install).

## Giới hạn trung thực

- **Chưa ký số (unsigned):** SmartScreen có thể cảnh báo "unknown publisher". Bỏ qua bằng "More info → Run anyway" chỉ khi bạn tin nguồn file.
- **Không auto-update:** upgrade = tải installer mới và cài đè (data giữ nguyên). Updater sẽ chỉ bật khi có endpoint + key custody + test N→N+1 thật.
- **Doc CBZ phổ thông hơn (F3a):** CBZ đọc được cả STORE và DEFLATE; app vẫn *xuất* STORE. Zip-bomb/entry lạ bị chặn có kiểm soát (`CBZ_INVALID`).
- **Nhập CBZ ngoài (F3b):** nút “Nhập CBZ” trong trang chi tiết truyện mở ộp chọn file native; tập tin được kiểm tra và chép vào thư viện dưới tên chuẩn hóa (người dùng tự chọn file hợp lệ — app không tự kiểm bản quyền nội dung đó).
- **Nguồn truyện:** app chỉ chuyện động tới host user đã đăng ký/allowlist; nguồn Madara có thể đăng ký kèm tối đa 4 *host ảnh phụ trợ* (exact-host, chỉ dùng cho request ảnh) — mọi request khác vẫn chỉ base host. Tôn trọng giới hạn nguồn (pace, retry budget). Không hỗ trợ DRM/paywall/login-wall.
- **Live-source + manual Windows checklist** chưa chạy trên máy sạch — xem ma trận trạng thái `RELEASE_TEST_MATRIX.md` (các mục BLOCKED/NOT RUN khai thẳng).
- Downgrade app sau khi DB đã migration forward-only **không được bảo đảm**; khôi phục bằng backup (runbook §Backup).

## Data & privacy

Toàn bộ dữ liệu (DB, library, cache) nằm trong `%APPDATA%\com.manga1989.downloader`. Không có telemetry, không upload thư viện, không account. Gỡ cài đặt **không** xóa dữ liệu người dùng.

- **Theo dõi chương mới (mặc định BẬT cho người dùng mới và sau nâng cấp — chủ đích, nêu rõ ở đây):** app định kỳ (~6 giờ khi đang chạy) chỉ lấy *metadata* danh sách chương của truyện bạn đã lưu, đúng qua host nguồn đã đăng ký với pacing sẵn có; không tải chương, không gửi gì đi. Tắt toàn cục trong Settings hoặc từng truyện trong trang chi tiết ("Theo cài đặt chung" = theo khóa toàn cục).

## Attribution

Stack: React + TypeScript + Vite · Tauri 2 (Rust/Tokio) · SQLite/SQLx. Binary chứa các crate/npm dependency nguồn mở theo license tương ứng (manifest trong `ARTIFACT_MANIFEST.md`).
