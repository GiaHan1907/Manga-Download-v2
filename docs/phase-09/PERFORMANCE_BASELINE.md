# Phase 09 — Performance Baseline

**Môi trường đo:** Windows 11 Pro x64 Build 26200, runner dev (2 CPU reported), SSD chưa xác nhận model, **debug build** (`cargo test` không `--release` — worst-case cố ý), Node v24.18.0. Mỗi phép đo median-of-3 (query) hoặc single-run (kHI chi phí ổn định). Test nằm trong suite thường: `perf.rs` — chạy lại được bằng `cargo test t09_perf -- --nocapture`.

## Số đo (capture 2026-09-23, commit candidate P09)

| Phép đo | Kết quả | Ngưỡng plan §P09-07 | Đánh giá |
|---|---|---|---|
| Library aggregate 1.000 manga × 10.000 chapters, sort title, filter downloaded, LIMIT 24 | **median 9.7 ms** | ~2 s meaningful content | ✅ vượt xa |
| Local search cùng dataset ("Perf Manga 999", lastRead sort) | **median 4.9 ms** | p95 ~300 ms | ✅ |
| Seed 11.001 INSERT một transaction batch | 8.4 s (fsync-bound) | — (chỉ phục vụ test) | Ghi chú: per-statement implicit commit đo ~48 s → lý do batch |
| CBZ 300 trang: `open()` (tail-window + central, 50 KB archive) | **18.9 ms** | on-demand, không pre-load | ✅ O(entries) |
| CBZ 300 trang: single page read cuối (page 299) | **0.12 ms** | — | ✅ seek-per-entry |
| Checkpoint: 2000 durable single-row updates (mỗi cái 1 commit — contract crash-safe) | **8.0 s (4.0 ms/write)** | không vi phạm integrity | ⚠ fsync chi phối; xem RISK R-3 |
| `tauri dev` startup tới window | ~10 s debug compile-cached, app hiện ổn định 35–105 s các smoke | cold/warm chưa đo chuẩn | NOT RUN (chuẩn hoá ở Phase 10) |
| RAM idle / đọc nhiều chương | chưa đo tự động (RISK R-4/47) | không growth không giới hạn | code-review evidence: blob LRU bán kính + revoke 3 đường; CBZ không đọc toàn file |

## Before/after các thay đổi

Phase 09 không đổi thuật toán nào để tối ưu (không có bottleneck nào đo được vượt ngưỡng). Thay đổi liên quan hiệu năng duy nhất: **log severity** (`io::Error` Display → kind) — không đáng kể. Trước đó P07 đã có tail-window parse + SNIFF_PREFIX=12; các con số trên xác nhận thiết kế đạt (18.9 ms/300 trang).

## Tripwires đã khoá trong suite

`assert` trong `perf.rs`: list <1500 ms, search <500 ms, cbz open <250 ms, page <50 ms, checkpoint <30 s — fail CI-local nếu regression bậc độ lớn.

## Chưa đo (giao Phase 10 / manual)

- p95 thật UI main-thread khi search (cần profiler trên release app).
- RAM curve đọc 10 chapter liên tiếp + 20-task download đồng thời (RISK R-4).
- Cold start release exe trên máy sạch (installer, mục R-2).
