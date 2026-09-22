# Phase 07 — Library & Reader API contract (P07-01)

Quy ước cũ giữ nguyên: mọi invoke qua `src/shared/api/desktop.ts`; input chỉ ID đã validate; camelCase DTO; lỗi envelope `{code,message,retryable}`.

## Migration `0005_library_reader.sql` (forward-only, ALTER thuần — không rebuild)

```sql
ALTER TABLE mangas   ADD COLUMN last_read_at TEXT;               -- touch khi save progress
ALTER TABLE chapters ADD COLUMN local_size_bytes INTEGER;        -- size artifact đã verify (reconcile/publish bổ sung)
ALTER TABLE reading_progress ADD COLUMN scroll_offset REAL;      -- resume vertical chính xác hơn
ALTER TABLE reading_progress ADD COLUMN page_total INTEGER;      -- % hoàn thành không phải mở archive
CREATE INDEX idx_mangas_last_read ON mangas(last_read_at);
```

## Library commands (module Rust mới `library/`, KHÔNG bảng mới)

| Command | Vào | Ra | Quy tắc |
|---|---|---|---|
| `library_list` | `{ query?, sort: 'lastRead'\|'recentDownload'\|'title'\|'chapters', filter: 'all'\|'downloaded'\|'partial'\|'favorite', limit, offset }` | `PagedResult<LibraryMangaDto>` | `LibraryMangaDto { id, title, author, status, favorite, downloadedChapters, totalChapters, localSizeBytes, lastReadAt, progress: {chapterId,pageIndex,pageTotal}? , hasMissing? }` — MỘT query aggregate (không N+1) |
| `library_get_manga` | `{ mangaId }` | `LibraryMangaDetailDto` (= LibraryManga + chapters: `LocalChapterDto[]` đã có, thêm `sizeBytes`, `artifactOk`) | artifactOk = Rust kiểm tại lúc gọi (bounded) |
| `library_reconcile` | — | `{ checked, healthy, missing, corrupted }` | chỉ ĐỌC file + clear cờ DB khi missing; corrupted giữ nguyên cờ-offline? không: corrupted ⇒ clear cờ + giữ file cho user tự xử; KHÔNG bao giờ xóa file |
| `library_remove_chapter` | `{ chapterId }` | `LocalChapterDto` (cờ đã clear) | path từ DB, validate relpath + under-root; xóa disk TRƯỚC, DB commit SAU; disk fail → `DELETE_FAILED`, DB nguyên |
| `library_remove_manga_files` | `{ mangaId }` | `{ removed: string[], failed: string[] }` | loop từng chapter như trên, partial-failure trung thực |

## Reader commands (module `reader/`)

| Command | Vào | Ra | Quy tắc |
|---|---|---|---|
| `reader_open_chapter` | `{ chapterId }` | `ReaderManifestDto { chapterId, mangaId, mangaTitle, chapterTitle, format, pageCount, pages:[{index,width?,height?}], prevChapterId, nextChapterId, startPage, startScrollOffset }` | chapter phải có local_path + artifact đọc được (`LOCAL_FILE_MISSING`/`CBZ_INVALID`/`LOCAL_FILE_CORRUPTED`); page_count + last_read_at được cập nhật; prev/next theo danh sách chapter TẢI-XONG của cùng manga, thứ tự như `list_chapters` |
| `reader_get_page_bytes` | `{ chapterId, pageIndex }` | **binary** (`ipc::Response`; JS: ArrayBuffer) | index trong manifest; đọc đúng 1 entry; entry path phải basename-only (chống traversal từ archive lạ); ext không phải ảnh ⇒ `UNSUPPORTED_IMAGE_FORMAT`; oversize cap `IMAGE_MAX_BYTES` |
| `app_save_reading_progress` (MỞ RỘNG, giữ tên cũ) | `+ scrollOffset?, pageTotal?` | `ReadingProgressDto` (+2 field) | vẫn validate ownership pair (Phase 04); đồng thời UPDATE `mangas.last_read_at` |
| `app_get_reading_progress` | giữ nguyên | +`scrollOffset,pageTotal` | |

Tiếp tục dùng `app_get/list` progress cũ cho Continue Reading; Home đọc `library_list(sort=lastRead, filter=downloaded, limit=…)`.

## Frontend

- `src/services/reader.api.ts` → `localServices.reader`: openChapter, getPageBytes (ArrayBuffer), saveProgress (debounce ở UI), getProgress.
- `src/services/library.api.ts` → `localServices.libraryV2` (hoặc mở rộng boundary hiện có): list/get/reconcile/remove.
- Route production: `/read/:chapterId`. Demo `/reader/...` giữ nguyên, có nhãn.
- Ảnh: `URL.createObjectURL(new Blob([buf]))` + LRU ≤ ~10 trang, revoke khi evict/unmount.
- CSP: thêm `blob:` vào `img-src`.

## Error codes mới (wire)

`LIBRARY_ITEM_NOT_FOUND, LOCAL_FILE_MISSING, LOCAL_FILE_CORRUPTED, CBZ_INVALID, UNSUPPORTED_IMAGE_FORMAT, PAGE_RESOURCE_NOT_FOUND, DELETE_FAILED, STORAGE_OUTSIDE_LIBRARY_ROOT` — retryable=false toàn bộ (trừ IO chưa chắc chắn: giữ false, user bấm lại).
