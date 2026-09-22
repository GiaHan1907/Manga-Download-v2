# Phase 07 — Test results

**OS:** Windows 11 x64 · **Node:** v24.18.0 · **Rust:** 1.98.1 stable-msvc · **Ngày:** 2026-09-23
Mọi test tự động dùng SQLite tạm + artifact CBZ/folder **thật** sinh bởi exporter Phase 06 (`reader/testkit.rs`) — không network.

## Quality gates (exit code thật)

| Lệnh | Kết quả |
|---|---|
| `npm run typecheck` / `npm run lint` / `npm run build` | **PASS** |
| `npm test` (vitest) | **PASS — 75/75** (9 files, +10 test Phase 07) |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --all-targets -- -D warnings` | **PASS (0 warning)** |
| `cargo test` | **PASS — 100/100** (mới phase này: 9 test `t07_*` backend + 6 unit `reader::*`/`sniff`) |
| `npm run tauri dev` (smoke) | **PASS** — app chạy ~35s ổn định, **DB upgrade schema 4→5 sạch**, không panic; đóng app chủ động |
| `npm run tauri build` | **NOT RUN** (gate đóng gói còn treo từ Phase 04) |

## Ma trận P07 (số §8–§10 plan, mapping test thật)

| Yêu cầu | Loại | Kết quả | Bằng chứng |
|---|---|---|---|
| Download completed → Library (flow §3) | FIXTURE end-to-end | PASS | `engine_tests::full_pipeline_produces_verified_cbz...` (Phase 06) sinh CBZ thật → `reader_service::tests::t07_cbz_manifest...` đọc lại manifest + từng page bytes đúng thứ tự |
| CBZ manifest: order, ignore non-image, path-unsafe reject | FIXTURE | PASS | `t07_cbz_manifest_order_sizes_neighbours_and_bytes`, `reader::tests::entry_name_guard`, `natural_sort_orders_numbers_not_lexically` |
| CBZ hỏng/garbage → lỗi kiểm soát | FIXTURE | PASS | `t07_missing_corrupt_and_traversal_paths_are_controlled` (CBZ_INVALID/FileMissing/Validation) |
| Image-folder manifest đồng nhất CBZ | FIXTURE | PASS | `t07_folder_mode_and_resume_position` |
| Per-page delivery (không nạp cả chapter) | CODE + unit | PASS | `cbz::open` chỉ đọc tail+central (segment); `read_entry` seek 1 entry; sniff cap 12 trang đầu |
| Natural sort deterministic | FIXTURE | PASS | `reader::tests::natural_sort...`, cbz entries sorted, folder list sorted |
| Progress save/restore + last_read_at + scroll bounds | FIXTURE | PASS | `t07_scroll_offset_and_page_total_roundtrip_and_last_read_touch` + resume start_page trong `t07_folder_mode_and_resume_position` |
| Prev/next theo chapter ĐÃ TẢI | FIXTURE | PASS | `t07_cbz_manifest...` (ch3 chưa tải bị loại khỏi chain) |
| Path validation mọi entry-point | FIXTURE | PASS | traversal reader + delete: `t07_remove_rejects_traversal_without_touching_disk` (file đích còn nguyên) |
| Delete chapter disk-first nhất quán | FIXTURE | PASS | `t07_remove_chapter_is_disk_first_and_path_safe`; partial-failure honest `t07_remove_manga_files_reports_partial_failure_honestly` |
| Reconcile non-destructive (missing/corrupted/healthy+size) | FIXTURE | PASS | `t07_reconcile_reports_missing_without_deleting_and_fixes_size` |
| Library aggregate counts/filter/sort/search escape | FIXTURE | PASS | `t07_list_aggregates_counts_favorite_progress_and_filters` |
| Upgrade schema 4→5 giữ dữ liệu | FIXTURE | PASS | toàn bộ test mở pool mới áp 0005; `tauri dev` smoke nâng DB thật |
| UI: 3 mode, RTL order, pairing, window prefetch | UNIT (jsdom) | PASS | `readerLogic.test.ts` 7/7; `phase07.test.tsx` mode-switch/double RTL dir |
| UI: navigation chuột+keyboards, chapter nav, resume 1/3 | UI | PASS | `phase07.test.tsx` reader UI (RTL ArrowLeft advance) |
| UI: controlled missing-image state | UI | PASS | `phase07.test.tsx` error test + `PageSlot` error state + retry |
| UI: Library states (empty/error/loading/filter rỗng) + truthfulness | UI | PASS | phase04 (cập nhật mock sang `library_list`) + phase05 truthfulness rewritten (0/2 offline ≠ downloaded) |
| Regression Phase 01–06 | — | PASS | vitest 75 (demo reader vẫn nguyên), cargo 100 |
| T07-24..32 integration/manual trên Windows (click X, fullscreen OS, 300-trang thật, offline net-off) | LIVE/MANUAL | **NOT RUN** | cần user trên máy; các risk đã có fixture tương đương |

## Trung thực

- Kích thước ảnh/zoom/fullscreen đã verify ở mức DOM unit + code path, **chưa** đo trên WebView2 thật (thuộc manual).
- CBZ **Stored-only** theo contract app-writer; archive deflate từ nguồn ngoài → `CBZ_INVALID` (defer inflate, ghi handoff).
- Live download (T06-24b) vẫn PENDING-MANUAL — flow §3 đầu-cuối chỉ khép kín khi user chạy live một lần.
