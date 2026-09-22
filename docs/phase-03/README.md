# Phase 03 — Interactive Prototype (UI Screens)

Toàn bộ nội dung Phase 03 là **DEMO / SAMPLE DATA**. Không có tìm kiếm Internet, tải truyện, lưu file, giải mã CBZ hay reader engine thật. Badge "Demo · dữ liệu mẫu" hiển thị thường trực trên topbar.

## Routes

| Route | Màn hình | Nguồn dữ liệu |
|---|---|---|
| `/` | Home Dashboard | mock services + SystemStatus (IPC thật Phase 01) |
| `/discover` | Tìm kiếm + Import URL demo | CatalogService (fixture, local-only) |
| `/manga/:mangaId` | Chi tiết + chọn chương | Catalog + Download + Library |
| `/library` · `/favorites` | Thư viện / yêu thích mẫu | LibraryService (mock state dùng chung) |
| `/downloads` | Hàng đợi demo + transitions | DownloadService |
| `/reader/:mangaId/:chapterId` | Sample Reader (SVG nội bộ) | ReaderService + ảnh `src/assets/sample-reader/` |
| `/settings` · `/settings/sources` | Cài đặt / nguồn demo | Settings + Catalog |
| Khác | 404 trong shell | — |

## Trạng thái UI

Mọi màn hình có: loading (Skeleton), success, empty (EmptyState hướng hành động), error recoverable (ErrorState). Control tương lai bị **disable kèm lý do** (Read sample chưa có trang mẫu, Thêm nguồn chờ Source Engine).

## Sample data disclaimer

- Catalog: 8 manga / 2 nguồn `src-alpha`, `src-beta`; timestamp và ID cố định.
- Ảnh reader: 5 SVG **tự sinh, trung tính** — không phải trang truyện bên thứ ba.
- Hàng đợi: progress theo **số trang mô phỏng**, không MB/s, không file thật.
- URL import: chỉ khớp `https://demo.manga1989.example/manga/<id>` trên text nội bộ; **không có network call nào tồn tại** trong code Path 03.
- State mock mất khi reload — hành vi chủ đích của prototype.

## Reset

Chỉ cho test/dev: `resetMockState()` từ `src/services/provider.ts`. Không có nút reset cho người dùng (tránh gợi ý backend thật).
