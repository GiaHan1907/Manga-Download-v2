# Phase 03 Screen Checklist

Trạng thái: [x] đã implement + có test tự động · [v] cần xác nhận trực quan trên desktop.

## Home `/`
- [x] Quick actions tới Discover và `?tab=url`
- [x] Stats chỉ từ fixture; storage = "—"
- [x] Continue Reading mở đúng `/reader/:mangaId/:chapterId` theo progress đã lưu
- [x] Recent Additions + Queue Preview điều hướng hợp lệ
- [x] SystemStatus (Phase 01) giữ nguyên độ thật

## Discover `/discover`
- [x] Debounce 300ms; kết quả cũ không ghi đè kết quả mới (alive-flag)
- [x] Lọc theo nguồn đang bật; empty state có action xóa bộ lọc
- [x] Import URL: 3 loại lỗi phân biệt (malformed / unsupported / unknown manga)
- [x] Match demo URL → preview → Detail; không network

## Manga Detail `/manga/:mangaId`
- [x] Hero 2:3, genres, status, nguồn demo
- [x] Chọn từng/bỏ/all-visible; sort ổn định
- [x] Queue báo added/skipped đúng; dedupe
- [x] Favorite toggle đồng bộ Library/Favorites/Home
- [x] Read chỉ với `hasSamplePages`; còn lại disabled + reason
- [x] Unknown id → error recoverable

## Library · Favorites
- [x] Grid/list, filter title/genre/status, sort title/recent
- [x] Remove qua Dialog xác nhận (mock-only)
- [x] Favorites dùng chung state; empty → CTA Discover

## Downloads `/downloads`
- [x] Filter All/Queued/Paused/Completed/Failed + search + sort
- [x] Transition đúng luật; invalid bị service từ chối, hiện thông báo
- [x] Progress trang mô phỏng, nhãn demo; không MB/s, không Open folder
- [x] View manga → Detail; state ổn định khi đổi route

## Reader `/reader/:mangaId/:chapterId`
- [x] Single/double/vertical, LTR/RTL, zoom clamp, fit width/page
- [x] Phím mũi tên/Home/End/+-/0/f/Escape; không bắt phím khi đang gõ input
- [x] Progress lưu mock state → Home phản ánh
- [x] Image fallback; cleanup listeners
- [v] Fullscreen/tương tác chuột thật trên desktop

## Settings · Sources
- [x] Validation 1–5 / 1–8; save/reset mock; output format "planned"
- [x] About hiển thị version thật qua IPC
- [x] Source enable/disable ảnh hưởng Discover; add-source disabled có lý do
- [v] Contrast và wrap tại 1024×700 / 125–200% scaling

## Cross-cutting
- [x] Badge "Demo · dữ liệu mẫu" thường trực
- [x] 404 + ErrorBoundary
- [x] Không invoke() ngoài desktop.ts; capability Phase 02 giữ nguyên
- [v] Keyboard sweep toàn trình + reduced-motion trên Windows
