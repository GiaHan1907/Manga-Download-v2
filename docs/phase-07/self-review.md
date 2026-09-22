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

7 gates + `tauri dev` smoke được chạy lại sau toàn bộ fix. Kết quả chi tiết + mục NOT RUN: `test-results.md`.

---

# Review 3 vòng (theo chỉ đạo user, sau commit `a6eccfc`)

## Review vòng 1 — backend (BLOCKING đã sửa)

- **R1-F1 — `image_folder::read_page` không cap kích thước.** CBZ chặn `MAX_PAGE_BYTES` khi đọc entry, nhưng folder mode `fs::read` toàn bộ file không giới hạn — một file ảnh cỡ lớn (thủ công/sync) sẽ bị nạp hết vào memory rồi mới reject. Sửa: guard `is_safe_entry_name` (defense-in-depth, thay guard thủ công) + check `metadata().len() > MAX_PAGE_BYTES` **trước khi đọc byte**, trả `LOCAL_FILE_CORRUPTED`. Test mới: `image_folder::tests::read_page_caps_size_and_rejects_unsafe_names` (cargo 101/101).
- Đã xác minh lại (không phải bug): `reading_progress` có `manga_id PRIMARY KEY` → 4 scalar subselect trong `library_list` luôn đọc cùng một row, không thể ghép lẫn hai bản ghi.

## Review vòng 2 — frontend + IPC (BLOCKING đã sửa)

- **R2-F1 — Đổi chapter qua `/read/:chapterId` giữ state cũ.** React Router tái dùng cùng component khi param đổi: `page`, cờ resume `initialized` và zoom rò từ chương trước → mở chương mới hiện sai trang và không resume. Sửa: tách `ReadPage` → `<ReaderSession key={chapterId}>`; mọi state per-chapter remount sạch. Test mới trong `phase07.test.tsx`: resume `3/3` theo `startPage`, rồi "Chương tiếp" phải về `1/5` (không phải `3/5`).
- **R2-F2 — Vertical mode: điều hướng bàn phím/pager không cuộn theo trang; scroll position không bao giờ được lưu.** Plan §Reader Mode A yêu cầu "restore approximate scroll position" — `scroll_offset` có đủ ở DB/IPC nhưng UI không bao giờ ghi/đọc. Sửa: `ReaderModes` thay IntersectionObserver bằng scroll handler tính `(page, fraction)` từ `offsetTop` trang đang chạm đỉnh viewport (CSS `.reader-viewport--vertical` thêm `position:relative`); effect programmatic-scroll cuộn tới trang khi page đổi **không** do user scroll (ref `scrollReported`), áp `startScrollOffset` một lần khi resume (`consumeResumeFraction`); `useReaderProgress` gửi `scrollOffset` (chỉ khi fraction còn khớp page đang ghi).
- **R2-F3 — Filter "Yêu thích" giấu truyện chưa tải.** Backend ghép thêm `AND downloaded_chapters > 0` trong khi plan định nghĩa filter là favorites; label UI không đề cập điều kiện tải. Sửa: filter favorite = đúng ngữ nghĩa favorites (`t07_list_...` vẫn PASS).
- Trivial: gỡ lỗi định dạng dính dòng ở `normalizeIpcError` (`desktop.ts`).

## Review vòng 3 — adversarial (BLOCKING đã sửa)

- **R3-F1 — Race kẹt trang vĩnh viễn trong `usePageImages`.** Cờ `cancelled` theo effect-run làm promise đang bay bị bỏ kết quả (không cache, không clear `pending`), còn `pending` lại không nằm trong deps effect → trang đã đánh dấu pending không bao giờ được fetch lại ở window mới; lật trang nhanh để lại placeholder loading vô hạn. Sửa: thay `cancelled`+`pending`-state bằng `inflight` **ref-set** — kết quả in-flight luôn vào cache khi chương vẫn khớp (`chapterRef`), window mới thấy `inflight` thì chờ chứ không đá trùng; hết trạng thái kẹt.
- **R3-F2 — `toggleFavorite` ở LocalMangaPage không catch** → unhandled rejection + im lặng khi IPC lỗi. Sửa: bắt lỗi, hiển thị qua `actionError` (pattern sẵn có của trang).

## Gates sau 3 vòng

typecheck/lint/build PASS · vitest **76/76** (thêm test resume/đổi chapter) · fmt/clippy PASS · cargo **101/101** (thêm test cap folder page) · `tauri dev` smoke xem `test-results.md`.

## Kết luận

3 BLOCKING giai đoạn implement + R1-F1 + R2-F1/F2/F3 + R3-F1/F2 đã sửa kèm test hoặc đường kiểm soát. **ACCEPT sau 3 vòng review.**
