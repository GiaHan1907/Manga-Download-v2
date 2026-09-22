# Phase 09 — QA Matrix (T09-01..T09-52)

**Commit candidate:** working tree trên `71c5ba8` (Phase 08 ACCEPT + review 3 vòng) + P09 changes.
**Runner:** Windows 11 Pro x64 Build 26200 · Node v24.18.0 · rustc 1.98.1 stable-msvc · debug build cho test, release cho build/smoke.
**Quy ước:** PASS = có output thật đã chạy; PASS(code) = logic test được, tác động OS chưa thao tác thật; PARTIAL = một phần kịch bản có bằng chứng; NOT RUN = chưa chạy; BLOCKED = thiếu môi trường/quyền. Không mục nào dịch BLOCKED/NOT RUN sang PASS.

| ID | Trạng thái | Lệnh / test | Actual / evidence |
|---|---|---|---|
| T09-01 | PASS | `npm run lint` · `typecheck` · `test` · `build` | 0 lỗi lint; tsc sạch; vitest 81/81 (10 files); vite build 396 kB; scripts đúng trong package.json |
| T09-02 | PASS | `cargo fmt --check` · `clippy --all-targets -- -D warnings` · `cargo test` | FMT_PASS; clippy 0 warning; **123/123** (toolchain rustc 1.98.1, deps khoá trong Cargo.lock) |
| T09-03 | PASS | `npm run tauri build` → EXIT=0; chạy `target/release/manga-downloader-1989.exe` | 2 bundles (MSI 6,459,392 B + NSIS 4,467,708 B, SHA256 trong `evidence/bundle-checksums-2026-09-23.txt`); release exe khởi chạy sống 12s, tray build không lỗi |
| T09-04 | PASS | vitest phase05 `search results+per-source errors` (fixture invoke) | Kết quả + lỗi từng nguồn hiển thị riêng; deterministic |
| T09-05 | PASS | vitest phase05 `URL-import rejection w/o fetch`, `link to remote preview`; Rust `madara_generic` resolve tests | Supported → preview; unsupported → lỗi nhãn đúng, không fetch |
| T09-06 | PASS | vitest phase05 `save-once distinguish saved/downloaded` + Rust `t04_04/t04_05` identity | Chapter list từ DB; lọc/sort `t04_11` pagination deterministic |
| T09-07 | PASS | Rust `t06_04 queue_batch bounded`, `t06_05 concurrent identical submit dedups` | Batch ≤100, duplicate reuses task |
| T09-08 | PASS | Rust `full_pipeline_produces_verified_cbz_and_completes`; UI `phase06 renders real tasks` | Verified CBZ → `chapters.local_path` set; Library aggregate đọc đúng cờ đó |
| T09-09 | PASS | vitest `phase08` invalidation map/store + `distinguishes truthful empty from DB error`; Rust upgrade tests | Completed event → bump library/home/downloads; mutations chéo màn qua domain bus; restart = same rows (T09-13) |
| T09-10 | PASS | Rust `t07_cbz_manifest_order_sizes_neighbours_and_bytes` | Mở CBZ offline, đúng thứ tự, không network |
| T09-11 | PASS | Rust `t07_folder_mode_and_resume_position` | Image-folder hoạt động như CBZ, resume vị trí |
| T09-12 | PASS | vitest `readerLogic.test.ts` (7) + `phase07` mode-switch/RTL + resume test | Logical page giữ nguyên khi đổi mode/direction; RTL trái-phải đúng |
| T09-13 | PASS | Rust `t04_08_settings_survive_reopen`, `progress_roundtrip`, `t09_corrupt_stored_blob_falls_back_to_defaults`; DB mở lại = test reopen | Favorites/progress/settings/history còn sau restart (pool mới trên cùng file) |
| T09-14 | PASS | vitest `phase08` palette (Ctrl+K, ↑↓ Enter Esc, XSS-inert, **không** gọi search_manga) | Offline local search, keyboard đủ |
| T09-15 | PASS | vitest app/phase03/04/07 error-state + phase08 favorites-error | Mọi cung loading/empty/error/retry có test; không dead-end (404 route test) |
| T09-16 | PASS | Rust `transient_failures_requeue_then_exhaust_to_failed`, `hostile_http200_html_page_fails_without_retry_burn`; timeout: `REQUEST_TIMEOUT` + `classify` network mapping | Không hoàn thành giả khi timeout/network lỗi — task về queued/failed thật |
| T09-17 | PASS | **MỚI `t09_classify_status_maps_http_codes_and_retry_after`** + engine exhaustion test | 401/403/404/429(Retry-After giữ nguyên)/5xx phân loại; retry budget ≤3, backoff có trần (`backoff_grows_and_stays_bounded`) |
| T09-18 | PARTIAL | `cancel/pause/resume` unit-per-state (engine_tests) — **20-task đồng thời chưa test tự động** | Từng transition có test race-free (revision guard); kịch bản 20 task = manual/defer, ghi RISK R-4 |
| T09-19 | PASS | `t06_05_concurrent_identical_submit_dedups`; frontend drop stale revision (phase06 L104); bridge bump idempotent (phase08) | Duplicate submit + event lặp đều an toàn |
| T09-20 | PASS | `t06_15_crash_after_page_rename_before_checkpoint_resumes` | Boundary A: trang đã rename nhưng chưa checkpoint → re-download, không mất dữ liệu đúng |
| T09-21 | PASS | `t06_17_cbz_roundtrip_ordered_verified` + `tampered_archive_fails_verification` + `.part` không bao giờ là dest hoàn tất (`part_paths` design) + recovery discard `.part` | `.part` không thể thành Completed |
| T09-22 | PASS | `t06_16_completed_task_with_vanished_file_clears_offline_flag` + adopt-if-verified (recovery comment path; engine `second_completed_download_never_overwrites_first_artifact`) | Crash sau rename trước DB commit → reconcile hoặc re-export idempotent, không overwrite |
| T09-23 | PARTIAL | permission-denied/disk-full: mọi IO error → `failed`(retryable)/`DELETE_FAILED` + log kind-only; **chưa mô phỏng thật đĩa đầy** | Cơ chế lỗi có test ở layer (io::Error mapping); scenario OS-level = manual |
| T09-24 | PASS | `t06_17` verify: đúng số trang, thứ tự tự nhiên, mở/giải nén được (STORE read-back + CRC) | Artifact verified đạt trước khi completed |
| T09-25 | PASS | `t06_12_stored_page_with_missing_file_resets_to_pending` | Checkpoint hỏng/thiếu → re-pending, không skip mù |
| T09-26 | PASS | `test_init_pool_creates_app_settings`, `test_init_pool_idempotent` | Fresh DB 0001→0005 sạch |
| T09-27 | PASS | `t04_02_upgrade_preserves_existing_app_settings_row`, `t06_01_phase05_upgrade_preserves_rows_and_widens_lifecycle` | Snapshot replay migration verbatim; rows còn nguyên |
| T09-28 | PASS | `fk_restrict_prevents_deleting_chapter_with_task_row`, `t04_04/05` unique, `t05_16_constraint_violation_rolls_back_entirely`, **MỚI `t09_concurrent_writers_serialize_without_busy_errors`** | FK/unique/txn/busy_timeout(5s) có bằng chứng; journal mode: mặc định rollback (theo tài liệu Phase 01 "không WAL") |
| T09-29 | PASS | `t07_reconcile_reports_missing_without_deleting_and_fixes_size` + recovery step 3 | Reconcile non-destructive, cờ trong sáng |
| T09-30 | PASS | `t07_remove_manga_files_reports_partial_failure_honestly`, `t07_remove_chapter_is_disk_first_and_path_safe` | Delete failure → `DELETE_FAILED`, DB nguyên, đường phục hồi = tải lại |
| T09-31 | PASS | `t08_clear_cache_keeps_active_and_removes_terminal_orphans` | Cleanup không đụng CBZ/progress; idempotent (assert vòng 2) |
| T09-32 | BLOCKED (by design) | Library root không đổi được ở P08 (chính sách đã ghi), ổ di động chưa rút-thật | Không có đường nào để mất file cũ vì không có đổi root; rút ổ = manual checklist Phase 10 |
| T09-33 | PASS | `t06_19_sanitizer_neutralizes_windows_hazards`, **MỚI `t09_unicode_titles_produce_safe_path_components`**, `relpath_rejects_traversal_and_absolute` | Unicode/reserved/dots an toàn ở mọi component |
| T09-34 | PASS | `rejects_credentials_and_ip_literals_and_private_targets` + **MỚI `t09_dns_guard_rejects_private_literals_and_unresolvable_hosts`** (loopback/private/link-local/metadata/CGNAT/ULA/NXDOMAIN fail-closed) | SSRF chặn tới tầng resolve |
| T09-35 | PASS | `redirect_targets_are_revalidated_before_the_next_hop` + allowlist exact-host mỗi hop | Redirect không thoát allowlist; DNS-rebinding bắt bởi guard trước connect |
| T09-36 | PASS | vitest phase05 hostile titles + phase08 palette XSS | Không HTML từ metadata thành code (React text) |
| T09-37 | PASS | **MỚI 7 cbz adversarial tests** (`t09_traversal_entry_name_rejects_archive_at_open`, deflate, truncation, count-cap, oversized-declared, non-image, empty) | Entry độc → CBZ_INVALID/loại, không đọc bytes |
| T09-38 | PASS | `t09_oversized_declared_entry_rejects_at_open_before_reading` + caps 10k/8MB/20MB + `MAX_PAGE_COUNT` 300, `IMAGE_MAX_BYTES` 20MB | Bomb-class giới hạn trước allocate |
| T09-39 | PASS | `t06_08_rejects_html_empty_truncated_oversized`, `html_body_never_lands_as_staged_page`, `valid_image_passes_and_kind_drives_extension`, folder-mode magic re-check | Ảnh giả MIME/extension bị từ chối |
| T09-40 | PASS | Inventory 38 commands (`docs/IPC_CONTRACT.md`) — tất cả typed id/index, không generic fetch/read/delete/execute; `deny_unknown_fields` settings | IPC surface đóng |
| T09-41 | PASS | `ensure_under_root_blocks_symlink_escape`, traversal delete tests, `dir_size` skip symlink | Canonical root mọi thao tác |
| T09-42 | PASS | Wire error `{code,message,retryable}` static VN messages; notification body chỉ title metadata; **MỚI** log sanitize `kind()` (S-2); smoke dev/release không in path | Không credentials/headers/path trong log/notify |
| T09-43 | PASS | npm audit 0 vuln; cargo audit: 1 advisory `rsa` (không có trong Windows tree — triage), 8 unmaintained warnings; capabilities/CSP đọc lại nguyên trạng | `evidence/audit-2026-09-23.txt`; không mở rộng capability |
| T09-44 | PASS | **MỚI `t09_perf_library_aggregates_and_search`** dataset 1.000 manga × 10.000 chapter | list median **9.7 ms**, search median **4.9 ms**, seed-batch 8.4 s (fsync-bound) |
| T09-45 | PASS | cùng test, query search qua idx + LIKE escape, chạy trong 5 ms | Dưới ngưỡng p95 300 ms của plan; UI main-thread: jsdom không đo → manual |
| T09-46 | PASS | **MỚI `t09_perf_cbz300_open_and_page_read_on_demand`** | open 300 trang = **18.9 ms** (không pre-load payload), page read = 0.12 ms, sniff cap 12 trang |
| T09-47 | PARTIAL | Code review: blob LRU có半径 + revoke 3 đường, CBZ tail-window O(entries); **RAM đo dài hơi chưa tự động** | Manual task manager checklist ở WINDOWS_SMOKE |
| T09-48 | PARTIAL | **MỚI `t09_perf_checkpoint_write_throughput`** = DB-contention half: 2000 durable checkpoint writes, 4.0 ms/write | 20-task end-to-end RAM/CPU/event-rate: cần fixture HTTPS local — manual/deferred, ghi RISK R-4 |
| T09-49 | NOT RUN | Resize/scaling/multi-monitor trên app thật | Checklist sẵn ở WINDOWS_SMOKE.md (cần người thao tác) |
| T09-50 | PASS(code) | Tray + close-to-tray + notification wiring compile + dev smoke 2×, release exe smoke; **thao tác chuột thật NOT RUN** | Hành vi trong `settings-and-storage-policy.md` §2 |
| T09-51 | PARTIAL | Keyboard: palette test (↑↓ Enter Esc), reader phím (phase07), dialog focus trap (phase04), visible focus CSS; fullscreen/escape thật = NOT RUN | reduced-motion: chưa implement — ghi issue R-6 |
| T09-52 | PASS | Full suite trên candidate: vitest 81/81, cargo 123/123, clippy/fmt/build sạch, tauri build EXIT=0, release exe chạy — xem mục Kết luận | Không Critical/High mở (see SECURITY_REVIEW §2, RISK_REGISTER) |

**Tổng:** PASS/PASS(code) 40 · PARTIAL 4 · NOT RUN 1 · BLOCKED-by-design 1 · (một số PASS có ghi chú manual-kèm). Không có FAIL mở.

**Live sources:** không chạy live journey nào trong Phase 09 — mọi E2E dùng fixture/mocked invoke theo rule §2.5; T06-24b/T08-36 vẫn PENDING-MANUAL.
