# Kiểm định cần người ngồi trước máy

Những mục dưới đây **không thể** kiểm bằng test tự động: chúng phụ thuộc vào cửa sổ thật, tray
thật, mạng thật hoặc cảm giác đọc. Mỗi mục ghi rõ cách chạy và hiện được đánh dấu thế nào.

Trạng thái hiện tại: **chưa chạy** (cần một buổi ngồi trước máy). Lần cuối các checklist này được
đánh NOT RUN là trong `docs/phase-07/test-results.md` và `docs/phase-08/test-results.md`.

## 1. Hiển thị và cửa sổ

| # | Việc | Cách chạy | Đạt khi |
| - | ---- | --------- | ------- |
| 1.1 | Scale 125% và 150% | Windows → Display → Scale, mở app lại | Chữ không bị cắt, nút không chồng, danh sách không tràn ngang |
| 1.2 | Cửa sổ nhỏ nhất 1024×700 | Kéo góc cửa sổ | Sidebar thu gọn được, lưới thẻ không tràn, không có scroll ngang |
| 1.3 | Toàn màn hình khi đọc | Mở chương → `F` | Vào/ra toàn màn hình sạch, thanh công cụ tự ẩn rồi hiện lại khi di chuột |
| 1.4 | Kéo/giảm cửa sổ bằng titlebar | Kéo vùng tiêu đề, double-click để phóng | Không nhảy giật, nút minimize/maximize/close đúng |

## 2. Tray và vòng đời

| # | Việc | Cách chạy | Đạt khi |
| - | ---- | --------- | ------- |
| 2.1 | Đóng xuống tray khi đang tải | Bật "Nút X thu nhỏ xuống tray", xếp hàng 1 chương, bấm X | Cửa sổ ẩn, tiến độ vẫn chạy (mở lại thấy tiến độ tăng) |
| 2.2 | Quit từ tray khi đang tải | Chuột phải tray → Quit | Worker dừng; mở lại app: chương được trả về hàng đợi, checkpoint còn nguyên |
| 2.3 | Thông báo chương xong | Để thông báo bật, tải xong 1 chương | Toast Windows hiện **một lần cho cả chương**, không phải mỗi ảnh |

## 3. Mạng và nguồn

| # | Việc | Cách chạy | Đạt khi |
| - | ---- | --------- | ------- |
| 3.1 | Ngắt mạng giữa chừng | Tải 1 chương, rút mạng khi đang chạy | Thẻ chuyển lỗi mạng, tự thử lại theo bậc thang; cắm mạng lại thì chương hoàn tất, không nhân đôi file |
| 3.2 | Nguồn trả lỗi tạm thời | Chờ một nguồn lỗi (hoặc dùng "Kiểm tra" để thấy mã) | Gợi ý bước tiếp theo hiện đúng theo mã, không chỉ có mã thô |
| 3.3 | Nguồn đổi tên miền | Sửa tên miền trong Nguồn → Kiểm tra lại | Truyện đã lưu vẫn trỏ đúng nguồn, tải được chương mới |

## 4. Đọc offline

| # | Việc | Cách chạy | Đạt khi |
| - | ---- | --------- | ------- |
| 4.1 | Chương 300 trang | Mở một chương dài | Mở nhanh (không đọc hết archive), cuộn mượt, không nhớ hết ảnh vào RAM |
| 4.2 | Đọc khi mất mạng | Ngắt mạng rồi mở chương đã tải | Đọc bình thường |
| 4.3 | Chương PDF | Mở truyện có chương PDF | Nút "Mở file" mở đúng file bằng ứng dụng mặc định của Windows |

## 5. Đã được test tự động che (không cần làm tay)

- Thứ tự và nhóm hàng đợi, mặc định bộ lọc, bộ dọn lịch sử: `tests/downloadsQueueOrder.test.tsx`,
  `tests/downloadsRetryInfo.test.tsx`, `tests/phase06.test.tsx`.
- Mở file chương ngoài app: `openable_artifact_resolves_under_the_root_and_refuses_bad_input` (Rust)
  và `tests/localMissingRedownload.test.tsx`.
- Nguồn: kiểm tra nguồn, gợi ý sửa tên miền: `tests/sourceProbe.test.tsx`.
- Sao lưu/khôi phục: 3 test trong `src-tauri/src/services/backup_service.rs`.
- Giới hạn băng thông: `src-tauri/src/downloader/limiter.rs` (số học thuần, không ngủ).
- Nhật ký hoạt động: `src-tauri/src/services/activity_service.rs` + `tests/activityFeed.test.tsx`.
- CBZ nén Deflate, chống zip-bomb: 10 test trong `src-tauri/src/reader/cbz.rs`.
- Hiệu năng đĩa/CBZ: `cargo test --lib perf::` (in số đo, có ngưỡng).