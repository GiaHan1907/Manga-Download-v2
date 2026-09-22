# Phase 07 — Self-review

Review đối chiếu plan `PHASE_07` (§8–§16) trên diff chưa commit. Phân loại BLOCKING (sửa ngay trong phase) / ADVISORY (ghi nhận, defer có chủ đích).

## Vòng 1 — backend correctness & boundaries (BLOCKING đã sửa)

- **F1 — `cbz::open` đọc nguyên file bằng `std::fs::read`.** Vi phạm chính ràng buộc của plan ("không bao giờ nạp cả chapter vào memory"): archive 300 trang có thể hàng trăm MB, và mỗi request một trang cũng re-read cả file. Sửa: parse EOCD bằng **tail window** (≤ 22 + 65.536 byte, `find_eocd` scan lùi có chặn) + chỉ đọc **central directory segment** qua `seek`; page bytes đọc lazy per-entry trong `read_entry` (vẫn cap `MAX_PAGE_BYTES` 20MB, STORE-only, re-validate local header).
- **F2 — `open_chapter` sniff kích thước MỌI page.** Mỗi lần mở chapter là đọc toàn bộ entry chỉ để lấy width/height. Sửa: `SNIFF_PREFIX = 12` — chỉ 12 trang đầu có `width/height`, còn lại `None`; UI hỗ trợ placeholder bất định (aspect-ratio khi biết, fallback khi chưa).
- **F3 — Progress ghi SQLite mỗi lần lật trang.** Cleanup của `useReaderProgress` từng flush theo `[page]` → mỗi phím mũi tên = 1 write DB, trái spec debounce. Sửa: chỉ ghi **debounced 800ms** khi đang đọc; **immediate flush** chỉ khi đổi chapter hoặc unmount (effect key `[chapterId]` + snapshot ref cập nhật sau commit — không đọc/ghi ref trong render).

## Advisories (không chặn, ghi nhận)

- **A1 — CBZ chỉ STORE.** Đúng contract app-writer Phase 06 (exporter luôn Stored); archive deflate từ nguồn ngoài → `CBZ_INVALID` có kiểm soát, không crash. Inflate defer — see handoff.
- **A2 — `read_entry` không đối chiếu CRC** local-header với central directory. Chấp nhận cho CBZ app-owned (cùng nguồn ghi); archive lạ vốn đã bị chặn ở central parse + name-guard + magic-check ảnh.
- **A3 — Xung đột nhịp giữa settings mặc định và toggle trong-reader**: Reader seed mode/direction từ `settingsApi.get` (Phase 04) nhưng override trên toolbar chỉ sống theo phiên — chủ đích (quyết định đang-đọc không phải preference); persist nếu user muốn ở phase sau.
- **A4 — Fullscreen**: gọi `requestFullscreen` thật, fallback CSS pseudo khi bị từ chối (jsdom/WebView2 chưa verify — mục manual).
- **A5 — Second page-count source**: `page_total` trong `reading_progress` được lưu nhưng resume tính theo manifest hiện tại phía Rust (`reader_service.rs` clamp `start_page` vào `0..page_count-1` trước khi trả), nên file bị reconcile-xóa giữa hai lần đọc không thể đẩy reader ra ngoài dải — đường resume nói chung có test (`t07_folder_mode_and_resume_position`).

## Boundary checks đã xác minh lại

- Renderer không bao giờ thấy đường dẫn: manifest chỉ mang index + kích thước; bytes đi `tauri::ipc::Response`. Test UI assert không có `.cbz` trong DOM.
- Mọi entry-point nhận ID từ renderer đều `validate_id` → path từ DB → `validate_relpath` + `ensure_under_root`; traversal trả `Validation`/`STORAGE_OUTSIDE_LIBRARY_ROOT` và **không chạm đĩa** (test khóa hành vi).
- Delete là disk-first: xóa file lỗi → `DELETE_FAILED`, DB nguyên; xóa được file mới clear DB.
- Reconcile non-destructive: corrupted giữ file trên đĩa (chỉ clear cờ) — người dùng tự quyết định xóa.

## Vòng 2/3 — gates sau mỗi lần sửa

7 gates + `tauri dev` smoke được chạy lại sau toàn bộ fix: typecheck/lint/build/vitest **75/75** · fmt/clippy/cargo **100/100** (9 test `t07_*` + 6 unit `reader::*`) · DB thật nâng schema 4→5 sạch. Kết quả chi tiết + mục NOT RUN: `test-results.md`.

## Kết luận

3 BLOCKING đã sửa kèm test khóa hành vi. **ACCEPT**, với các caveat NOT RUN/LIVE đã khai thẳng thắn ở `test-results.md` §Trung thực.
