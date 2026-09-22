# Phase 07 — Reconnaissance (P07-01)

**Ngày:** 2026-09-23 · **Base:** `2c92ea9` (Phase 06 + review rounds), tree sạch, ahead 2 chưa push.

## 1. Input thật từ Phase 06 (không tin trí nhớ — đã đối chiếu code)

- Chapter đã tải = `chapters.local_path IS NOT NULL` (relpath dưới `app_data/library/`, format `output_format ∈ {cbz, folder}`); chỉ ghi SAU khi artifact verify (worker publish_completed) và startup recovery dọn cờ giả (file mất → NULL).
- CBZ của app: ZIP **Stored**, entry basename-only zero-padded `NNNNNN.<ext>` (thứ tự từ manifest), DOS-date cố định. Folder: thư mục chứa đúng các file `NNNNNN.<ext>` đã verify.
- `download_tasks` là lịch sử hàng đợi, không phải nguồn chân lý cho "đã có offline".
- Schema hiện tại: `reading_progress(manga_id PK, chapter_id, page_index, updated_at)` — MỘT dòng/manga, **chưa có** scroll_offset/page_total; `mangas` chưa có last_read_at; `chapters` chưa có kích thước file.
- Index sẵn có: `idx_chapters_manga`, `idx_progress_updated`, `idx_mangas_updated_at`.
- Error: `AppError{code,message,retryable}`; taxonomy source-side đã tách; reader/library cần thêm code mới (§4 dưới).
- CSP: `img-src 'self' asset: http://asset.localhost`; capability window `main`: core:default + 5 window perms — **không fs/shell/http** (bất biến §3.2 HANDOFF).

## 2. Nghĩa chuẩn (canonical) của các trạng thái

| Khái niệm | Định nghĩa Rust-side |
|---|---|
| downloaded | `chapters.local_path != NULL` **và** artifact tồn tại + verify được khi đọc/reconcile |
| completed (task) | `download_tasks.status='completed'` — lịch sử, không suy ra offline hiện tại |
| missing | DB có local_path nhưng file/thư mục không tồn tại → reconcile trả `missing`, clear cờ, giữ metadata |
| corrupted | Tồn tại file nhưng parse fail (EOCD/central sai, entry thiếu CRC, folder rỗng) → `corrupted`, KHÔNG tự xóa |
| orphaned | File trong `library/<manga-dir>/` không khớp bất kỳ chapter nào (tên sinh theo task-token) → chỉ BÁO, không xóa |

## 3. Chiến lược resource-delivery (quyết định, thay vì custom protocol)

`reader_get_page_bytes(chapterId, pageIndex)` → `tauri::ipc::Response(Vec<u8>)` (binary IPC Tauri 2, JS nhận ArrayBuffer). Lý do: không cần protocol/CSP mới cho scheme lạ, không đường dẫn nào rời Rust, mỗi lần đúng 1 trang (không base64 nguyên chapter). Frontend: `URL.createObjectURL(blob)` + LRU có thu hồi `revokeObjectURL`. **CSP phải thêm `blob:` vào img-src** (duyệt trong phase này, có chủ đích, ghi handoff).

Manifest không chứa tên file/đường dẫn — chỉ `{index, width?, height?}`; resource_id ≡ (chapterId, index).

## 4. Lỗi mới (taxonomy wire code, message tiếng Việt sanitize)

`LIBRARY_ITEM_NOT_FOUND`, `LOCAL_FILE_MISSING`, `LOCAL_FILE_CORRUPTED`, `CBZ_INVALID`, `UNSUPPORTED_IMAGE_FORMAT`, `PAGE_RESOURCE_NOT_FOUND`, `DELETE_FAILED`, `STORAGE_OUTSIDE_LIBRARY_ROOT` — qua `AppError::Library(LibraryError)` mới, giữ envelope cũ.

## 5. Route & phân vùng demo/production (giữ regression Phase 03)

- Production reader: route MỚI `/read/:chapterId` (chapterId = id local đã validate).
- Demo reader cũ `/reader/:mangaId/:chapterId` GIỮ NGUYÊN + nhãn demo (mock SVG fixtures; T03-08 vẫn phủ). Home/Library production chỉ trỏ vào `/read/...`.
- Reader preferences mặc định đọc từ Settings thật (`defaultReaderMode`, `defaultReadingDirection` — Phase 04).

## 6. Không tạo khái niệm trùng

Không có `library_chapters`/`reading_state` thứ hai: đọc thẳng `chapters` (+`mangas`), `reading_progress` mở rộng. `library_get_all` là VIEW tổng hợp từ tables có sẵn, không bảng mới.
