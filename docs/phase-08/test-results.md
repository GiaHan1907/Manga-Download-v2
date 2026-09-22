# Phase 08 — Test results

**OS:** Windows 11 x64 · **Node:** v24.18.0 · **Rust:** 1.98.1 stable-msvc · **Ngày:** 2026-09-23

## Quality gates (exit code thật, chạy sau commit docs này)

| Lệnh | Kết quả |
|---|---|
| `npm run typecheck` / `npm run lint` / `npm run build` | **PASS** |
| `npm test` (vitest) | **PASS — 81/81** (10 files; +5 test `phase08.test.tsx`, các test phase03/04/06 đã cập nhật theo UI Phase 08 có chủ đích) |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --all-targets -- -D warnings` | **PASS (0 warning)** |
| `cargo test` | **PASS — 104/104** (+3 test `t08_*`) |
| `npm run tauri dev` (smoke) | **PASS** — chạy ~105s ổn định với **tray + tauri-plugin-notification mới**, 0 error/panic trong log, DB schema 5; đóng app chủ động |
| `npm run tauri build` | **NOT RUN** (gate đóng gói treo từ Phase 04 — thuộc phạm vi release, không phải Phase 08) |

## Ma trận T08-01..T08-36

| ID | Kịch bản | Loại | Kết quả | Bằng chứng |
|---|---|---|---|---|
| T08-01 | DB Phase 07 → khởi động Phase 08, không mất dữ liệu | FIXTURE | PASS | Toàn bộ `cargo test` mở pool mới qua migrations 0001–0005 (favorites/progress/tasks/settings đọc lại); smoke `tauri dev` trên DB thật |
| T08-02 | Discover → Detail → queue với ID đúng | FIXTURE (P05/P06 hồi quy) + UI | PASS | suite phase05/phase06 cũ green; queue payload chỉ mang chapterIds |
| T08-03 | Task completed → mọi màn cập nhật không restart | UNIT + wiring | PASS (mức wiring) | `domainsForTaskStatus("completed")` = downloads+library+home; `AppEventBridge` subscribe 1 lần; `useDomainVersion` refetch (test store). **Hiệu ứng thật end-to-end cần 2 cửa sổ / running app = NOT RUN (manual)** |
| T08-04 | failed/cancelled phản ánh trạng thái backend | UNIT | PASS | map + snapshot-refetch; engine source-of-truth không đổi (hồi quy P06) |
| T08-05 | Duplicate queue không nhân bản | FIXTURE | PASS | `queue_batch` dedup P06 (test cũ green) |
| T08-06 | Mở chương offline không cần mạng | FIXTURE | PASS | `t07_cbz_manifest...`/`t07_folder_mode...` (P07), không network |
| T08-07 | Đọc trang → Continue Reading cập nhật | UNIT + wiring | PASS (mức wiring) | `useReaderProgress` thành công → `bumpDomain("home"/"library")`; strip Home đọc `library_list` theo domain |
| T08-08 | Restart app còn favorites/progress/settings/history | FIXTURE | PASS | `t04_08_settings_survive_reopen...`, `progress_roundtrip...`, task rows trong DB; smoke restart |
| T08-09 | Xóa chapter đang đọc → fallback kiểm soát | FIXTURE | PASS | `library_list` giờ ẩn progress trỏ chapter chưa có artifact (`t08_stats_and_progress_fallback...`); reader mở chapter mất file → `LOCAL_FILE_MISSING` (`t07_missing_corrupt...`) |
| T08-10 | Ctrl+K + keyboard navigation | UI | PASS | `phase08.test.tsx` palette (mở bằng phím, ↑↓ đổi aria-selected, Enter điều hướng, Esc đóng) |
| T08-11 | Tìm kiếm offline, không tự phát remote request | UI | PASS | test assert `library_list` được gọi, `search_manga` KHÔNG được gọi |
| T08-12 | Title HTML-like render inert | UI | PASS | assert không sinh `img` trong palette, text hiển thị nguyên vẹn |
| T08-13 | Favorite từ nhiều màn đồng bộ | FIXTURE + wiring | PASS | cùng `app_set_manga_favorite`; bump favorites/library/home; UI tests phase04/07 green |
| T08-14 | Metadata refresh không reset favorite/progress/paths | FIXTURE | PASS | upsert P05 giữ cờ (test `t05_*` cũ green — không đổi đường này) |
| T08-15 | History pagination/filter | UI + FIXTURE | PASS | FavoritesPage分页 test; DownloadsPage trang 100 theo `total` server (header 2/2 test phase06 cập nhật) |
| T08-16 | Clear history có xác nhận, không xóa CBZ | — | **NOT IMPLEMENTED (có chủ đích)** | xem policy §3 — nút nửa vời vi phạm DoD; deferred |
| T08-17 | Concurrency settings validate + tôn trọng giới hạn nguồn | FIXTURE | PASS | bounds Rust (`bounds_are_enforced`) + scheduler clamp (P06) + adapter capability gate (P05/P06) |
| T08-18 | PDF không hiển thị như option | UI | PASS | SettingsPage bỏ option PDF + chú thích; engine từ chối pdf tại queue (test P06) |
| T08-19 | Disable source không mất dữ liệu local | FIXTURE | PASS | `set_source_enabled` chỉ đổi `sources.enabled` (schema); test P05 |
| T08-20 | Đổi library root new-only | — | **NOT IMPLEMENTED (có chủ đích)** | Không cho đổi root ở P08 (policy §3) — không có furnish move an toàn |
| T08-21 | Clear cache với manga completed không mất file/progress | FIXTURE | PASS | `t08_clear_cache...` — staging task terminal bị xóa, library/artifact không đụng |
| T08-22 | Cache cleanup khi task active | FIXTURE | PASS | test giữ `t-active`/`t-downloading`; `kept_active_dirs` báo cáo đúng |
| T08-23 | Symlink/traversal bị từ chối | FIXTURE | PASS | `t07_remove_rejects_traversal...`, `entry_name_guard`, `ensure_under_root_blocks_symlink_escape`, `dir_size` bỏ qua symlink |
| T08-24 | Disk missing/full → lỗi rõ, không fake completed | FIXTURE + code | PASS (fixture) | `root_available:false` render cảnh báo; error taxonomy P06 giữ; **bẻ ổ đĩa thật = NOT RUN (manual)** |
| T08-25 | Notification bật/tắt theo preference | CODE + smoke | PASS (mức code) | `RuntimeFlags.notify_enabled` chặn ở sink; plugin build pass trong smoke; **toast thật trên Windows = NOT RUN (manual, cần download live)** |
| T08-26 | Đóng app khi đang tải | CODE | PASS (mức code) | close-to-tray policy + ExitRequested shutdown (không đổi semantics P06); **click-X thật với download live = PENDING-MANUAL (kèm T06-24b)** |
| T08-27 | Tray Quit vs Hide | CODE + smoke | PASS (mức code/smoke) | Tray xây thành công trong smoke, menu Show/Downloads/Quit wired (`app.exit(0)` đi qua lifecycle); **thao tác chuột thật = NOT RUN (manual)** |
| T08-28 | App offline: local chạy, remote báo rõ | FIXTURE + UI | PASS | reader/library/stats không network; LiveSearchPanel lỗi nguồn đã test (P05) |
| T08-29 | High-DPI + min window | — | **NOT RUN** | manual Phase 09 checklist |
| T08-30 | Unicode/HTML metadata lớn | UI + FIXTURE | PASS | palette test + React render text mặc định; slug P06 sanitize tests |
| T08-31 | Command input abuse | FIXTURE | PASS | bounds `limit/offset/page_index/batch` + `deny_unknown_fields` (tests cũ); `validate_id` mọi entry |
| T08-32 | Hồi quy Phase 06 | FIXTURE | PASS | cargo engine_tests + vitest phase06 green sau đổi pagination |
| T08-33 | Hồi quy Phase 07 | FIXTURE | PASS | `t07_*` + vitest phase07 green |
| T08-34 | Fresh DB + upgrade | FIXTURE | PASS | migrations không thêm file mới (chỉ settings JSON key mới); toàn bộ test chạy fresh pool; DB smoke giữ schema 5 |
| T08-35 | Event unsubscribe/reconnect | CODE + test | PASS | bridge cleanup `disposed` + unlisten list; listen mock verify; hook per-screen không nhân listener (chỉ bridge subscribe app-level) |
| T08-36 | Journey live trên nguồn được phép | LIVE | **BLOCKED/NOT RUN** | như T06-24b: mạng dev chặn; cần user chạy — KHÔNG tuyên bố PASS |

## Trung thực

- Các mục "PASS (mức wiring/code)" nghĩa là: logic được unit-test và đường IPC đã nối; hiệu ứng nhìn-thấy trên Windows thật (toast, tray click, close-to-tray) mới nằm trong smoke build chạy được, chưa thao tác thủ công → Phase 09 nên checklist lại.
- Notification chỉ bật từ Rust path → capabilities không đổi (`core:default` + window perms), không thêm fs/shell/opener.
- "Open folder" và "đổi library root" và "clear history": không có nút giả; lý do ghi trong `settings-and-storage-policy.md`.
- Live download (T08-36/T06-24b) vẫn BLOCKED — pipeline chỉ khép kín end-to-end khi user chạy một lần trên mạng thật.
