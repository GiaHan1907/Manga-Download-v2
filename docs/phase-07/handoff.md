# Phase 07 — Handoff

**Ngày:** 2026-09-23 · **Commit local:** `feat: Phase 07 ...` (xem `git log`, chưa push — theo quy trình)
**Range:** migration `0005_library_reader.sql` · `src-tauri/src/reader/` · `services/{library_service,reader_service}.rs` · `commands/{library,reader}.rs` · `src/features/{library,read}/` · 7 IPC commands mới.

## Giao những gì

1. **Library backend thật** (`library_list`, `library_get_downloaded_chapters`, `library_reconcile`, `library_remove_chapter`, `library_remove_manga_files`): aggregate một câu SQL (số chương đã tải/tổng, dung lượng, progress, has-missing), 4 sort + 4 filter + search LIKE-escape; delete disk-first path-safe; reconcile non-destructive.
2. **Reader backend** (`reader_open_chapter`, `reader_get_page_bytes`): manifest thống nhất cho CBZ (tail-window central-dir, STORE-only) và image-folder; page bytes per-page qua `tauri::ipc::Response` (ArrayBuffer) — không bao giờ base64 cả file, không bao giờ lộ đường dẫn.
3. **UI Library chi tiết**: grid/list (nhớ lựa chọn qua localStorage), sort/filter, trạng thái empty/error/loading phân biệt thật, badge "Có file đã mất", xóa chương với confirm inline + báo cáo partial trung thực.
4. **Offline Reader** (`/read/:chapterId`): 3 chế độ (vertical / single / double-page), LTR + RTL, zoom, fullscreen (thật → fallback CSS), keyboard đầy đủ (mũi tên theo direction, PgUp/Dn, Home/End, +/-/0, T, F, M, Esc), toolbar auto-hide, prefetch cửa sổ trang + blob LRU revoke.
5. **Progress bền**: debounced 800ms + flush tức thời khi đổi chapter/unmount; `last_read_at` trên `mangas` → "Đọc tiếp" ở Home (strip dữ liệu THẬT, tách khỏi panel demo) và detail local; resume đúng trang + scroll offset.
6. Schema 5 (forward-only ALTER + index); toàn bộ gate xanh — chi tiết `test-results.md`; quyết định kỹ thuật — `library-reader-contract.md`, `integration-notes.md`; lỗi review đã sửa — `self-review.md`.

## Resource strategy (điểm dễ hiểu sai)

- 1 request = 1 page. Frontend tự quản cửa sổ prefetch (bán kính 3 trang ở vertical, 1 ở mode còn lại) và cache blob theo chapter; Rust không giữ state reader nào giữa hai request.
- CBZ parse rẻ và stateless (chỉ đọc segment cần), nên worker/pool không cần thiết ở quy mô desktop; nếu profile sau này cho thấy re-parse là điểm nóng, thêm handle cache — chưa làm vì chưa đo.

## Known issues / đã ghi nhận

- CBZ **Stored-only**: archive deflate từ nguồn ngoài → `CBZ_INVALID` (lỗi kiểm soát, retryable=false).
- `reader_get_page_bytes` mở file mỗi lần gọi — đủ nhanh cho app-local, nhưng là ứng viên số 1 nếu đo thấy chậm với đĩa chậm.
- Sniff kích thước chỉ 12 trang đầu; các trang sau `width/height: null` cho tới khi tải (UI phải xử null — đã xử lý).
- Fullscreen/IME/kích-thước thật chưa verify trên WebView2 đóng gói (manual, xem `test-results.md` NOT RUN).

## Deferred sang Phase 08+

- `npm run tauri build` + đóng gói/cài đặt (gate treo từ Phase 04).
- Live download→read khép kín trên máy user (T06-24b vẫn PENDING-MANUAL).
- Inflate cho CBZ deflate (nếu cần mở archive ngoài).
- Persist mode/direction đổi từ toolbar reader vào Settings (hiện chỉ theo phiên — xem self-review A3).
- Command palette, cover-art fetch cho card Library/Continue, page-count badge theo thời gian thực.

## Bất biến phải giữ khi làm tiếp

1. Renderer chỉ gửi ID/index; đường dẫn và URL tài nguyên không bao giờ qua IPC (capability không fs — đừng bao giờ thêm fs scope để "cho nhanh").
2. Mọi đường vào file người-trigger: `validate_id` → path từ DB → `validate_relpath` → `ensure_under_root`.
3. Xóa = disk-first, lỗi đĩa thì không đụng DB; reconcile không bao giờ xóa file của user.
4. Progress writes phải debounce; flush tức thời chỉ ở ranh giới chapter/unmount.
5. Gate trước commit: typecheck · lint · build · vitest · fmt · clippy `-D warnings` · cargo test.
