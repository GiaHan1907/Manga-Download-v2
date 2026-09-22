# Phase 07 — Integration notes (diff thật)

**Base:** `2c92ea9` · **Ngày:** 2026-09-23

## Backend (Rust)

| File | Nội dung |
|---|---|
| `migrations/0005_library_reader.sql` | ALTER thuần, forward-only: `mangas.last_read_at`, `chapters.local_size_bytes`, `reading_progress.scroll_offset/page_total`, index `idx_mangas_last_read`. Không bảng mới, không đụng dữ liệu cũ |
| `error.rs` | `LibraryError` (8 mã wire §12 brief: LIBRARY_ITEM_NOT_FOUND…STORAGE_OUTSIDE_LIBRARY_ROOT), message tĩnh tiếng Việt, retryable=false |
| `reader/` (module mới) | `mod.rs` (allowlist ext, basename-only guard, natural sort), `cbz.rs` (parse EOCD+central directory STORE-only, đọc 1 entry bằng seek — không nạp cả archive; entry tên có `/`,`\`,`:`… ⇒ CBZ_INVALID; method≠0 ⇒ CBZ_INVALID), `sniff.rs` (PNG/JPEG/GIF/WebP dimensions từ header), `image_folder.rs` (non-recursive, bỏ symlink, natural sort), `testkit.rs` (build CBZ/folder thật bằng exporter cho test) |
| `services/reader_service.rs` | `open_chapter` (validate id → local_path từ DB → validate_relpath + ensure_under_root → manifest + sniff dims; page_count persisted; neighbours = chương **đã tải** cùng manga theo đúng thứ tự list_chapters; resume position từ reading_progress cùng chapter), `page_bytes` per-page, mọi lỗi có kiểm soát |
| `services/library_service.rs` | `list_mangas` aggregate 1 query (không N+1): downloaded/total counts, size, last_download, progress, has_missing — revised 4 vòng: chỉ khi `task completed VÀ output_relpath còn (mất artifact thật/externally-deleted) mà local_path NULL`; in-app delete clear cả `output_relpath` ⇒ không bị báo missing; `page_count` là metadata catalog, delete files không xóa), 4 sort + 4 filter + search escape LIKE; `downloaded_chapters` (+artifactOk fs-check); `reconcile` non-destructive (missing⇒clear cờ, corrupted⇒clear cờ + giữ file, healthy⇒backfill size); `remove_chapter_files` **disk-first** (xóa fail ⇒ DELETE_FAILED, DB nguyên; path validate + under-root); `remove_manga_files` per-item honest (removed/failed) |
| `services/progress_repo` + command | save_progress thêm scroll_offset(0..=1)/page_total(1..=10000) + stamp `mangas.last_read_at`; giữ nguyên validate ownership Phase 04 |
| `commands/{library,reader}.rs` | 5 library + 2 reader commands; `reader_get_page_bytes` trả `tauri::ipc::Response` binary (ArrayBuffer phía JS) |
| `app_state.rs` / `lib.rs` | `AppState.app_data_dir`; register 7 commands; **shutdown hook ExitRequested giữ nguyên** |

## Frontend

| File | Nội dung |
|---|---|
| `types/library.ts` | Mirror DTO Serde (LibraryMangaDto, DownloadedChapterDto, ReconcileReportDto, ReaderManifestDto…) |
| `shared/api/desktop.ts` | 7 wrapper mới; `readerGetPageBytes` normalize ArrayBuffer/Uint8Array/number[] → Uint8Array; error codes mới vào set; `saveReadingProgress` +scrollOffset/pageTotal |
| `services/library.api.ts`, `services/reader.api.ts`, provider | Boundary production `localServices.libraryV2/reader` — mọi lời gọi vẫn qua `desktop.ts` (bất biến §3.1) |
| `features/read/*` | `readerLogic.ts` (spreads/pairing, RTL order, prefetch window, clamp) — **pure, unit-tested**; `usePageImages.ts` blob LRU: fetch theo window, **revokeObjectURL** khi evict/đổi chapter/unmount; `useReaderProgress.ts` debounce 800ms + immediate trên unmount/đổi chapter; `ReaderModes.tsx` (single click-zones theo direction, double spread theo dir, vertical IntersectionObserver-guarded), `ReadPage.tsx` (manifest, resume startPage, keyboard §10 gồm RTL-swap, zoom 0.5–3, fullscreen + fallback, toolbar auto-hide 3s, prev/next chapter) |
| Route | `/read/:chapterId` production; demo `/reader/...` giữ nguyên nhãn |
| `LibraryPage` | library_list: sort/filter/search/grid-list (localStorage **chỉ preference**, không phải dữ liệu), count offline + size + progress %, badge "Chưa tải chương nào" trung thực, nút **Kiểm tra file** (reconcile report), hasMissing cảnh báo |
| `LocalMangaPage` | Section "Đã tải" (size, format, artifactOk/missing), Đọc + Đọc tiếp (`/read/`), **delete có xác nhận** per-chapter + per-manga (giữ metadata/fav/progress), queue giữ nguyên Phase 06 |
| `HomePage` | `LocalContinueReading` thật (last-read + %) tách khỏi panel demo |
| `contracts.ts`/`setup.ts`/CSS | 8 error code; URL.createObjectURL polyfill test; Dark Purple tokens reused (`--surface`, `--accent`, pill) |

## CSP / capability (kê khai thay đổi được brief cho phép)

- `img-src` thêm **`blob:`** (reader cần object-URL). Không thêm fs/shell/http, không đổi capabilities (binary qua command sẵn có).

## Lệch so với plan (có chủ đích)

1. Plan gợi ý module `library/{service,repository}.rs` + `reader/{service,manifest,cbz,image_folder,progress,models}.rs` → repo này có quy ước `db/repositories` + `services/`; giữ `reader/` thuần archive-logic + `services/{reader,library}_service.rs` theo kiến trúc đã accepted (§5: “don’t force exact filenames”).
2. `reader_get_manifest` + `reader_get_page_resource` nhập lại thành `reader_open_chapter` + `reader_get_page_bytes` (binary IPC, không token loop).
3. ZIP **Stored-only** (đúng contract Phase 06 writer); CBZ deflate ngoài contract ⇒ `CBZ_INVALID` có kiểm soát (không thêm dependency inflate — defer, ghi handoff).
4. Continue Reading UI Home: strip thật riêng, không thay panel demo (labeling Phase 03 giữ).
5. Không có `library_get_all` tách riêng — `library_list` là API aggregate (mapping §8 của plan chỉ là logical).

## Không phá bất biến

Không đường dẫn nào vào renderer; không command URL/fs tùy ý; một SQLite pool; migration append-only (`0001..0003` không đổi); demo fixtures vẫn cô lập; capability không mở rộng ngoài `blob:` CSP.
