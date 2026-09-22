# Phase 06 — Test results

**OS:** Windows 11 (10.0.26200) x64 · **Node:** v24.18.0 · **Rust:** 1.98.1 (stable-msvc) · **Ngày:** 2026-09-22
**Nguồn bằng chứng:** fixture/scripted adapter + SQLite tạm cho mọi test tự động; KHÔNG dùng live network trong test unit. Cột "loại" phân biệt FIXTURE vs LIVE.

## Quality gates (exit code thật, chạy từ repo root)

| Lệnh | Kết quả |
|---|---|
| `npm run typecheck` (`tsc -b`) | **PASS** (exit 0) |
| `npm run lint` (`eslint .`) | **PASS** |
| `npm run build` (`tsc -b && vite build`) | **PASS** |
| `npm test` (`vitest run`) | **PASS** — 65 passed / 0 failed (7 files) |
| `cargo fmt --all -- --check` | **PASS** |
| `cargo clippy --all-targets -- -D warnings` | **PASS** (0 warning) |
| `cargo test` | **PASS** — 83 passed / 0 failed |
| `npm run tauri dev` (smoke) | **PASS** — app chạy, **DB upgrade schema 3→4 thành công** (không panic), dispatcher + recovery khởi động sạch; đóng app exit 0 |
| `npm run tauri build` (đóng gói MSI/NSIS) | **NOT RUN** trong phiên này (gate bắt buộc trước release — xem handoff) |

## Ma trận T06-01..24 (tự động, trừ khi ghi khác)

| ID | Kịch bản | Loại | Kết quả | Bằng chứng (test) |
|---|---|---|---|---|
| T06-01 | Upgrade DB Phase 05 → 0004, giữ catalog/fav/progress | FIXTURE | PASS | `db::tests::t06_01_phase05_upgrade_preserves_rows_and_widens_lifecycle` |
| T06-02 | Chapter fixture → manifest identity ổn định, ordered | FIXTURE | PASS | `madara_generic::page_manifest_is_ordered_absolute_https_and_deduped` |
| T06-03 | Nguồn thiếu capability → không fetch, lỗi an toàn | FIXTURE | PASS | `engine_tests::unsupported_and_disabled_sources_fail_safely` |
| T06-04 | Chọn 1/N/all → task id + chặn batch >100 | FIXTURE | PASS | `download_repo::t06_04_queue_batch_creates_bounded_tasks_and_rejects_oversize` |
| T06-05 | Double/concurrent identical submit → dedup 1 task | FIXTURE | PASS | `download_repo::t06_05_concurrent_identical_submit_dedups_to_one_active_task` |
| T06-06 | Nhiều chapter/source/host → bound + fair | FIXTURE | PASS (bounded) | `scheduler` capacity clamp 1..5 + `progress` coalesce; `engine_tests` pause/resume |
| T06-07 | JPG/PNG/WebP hợp lệ → staged match type/index | FIXTURE | PASS | `transfer::t06_07_fetch_page_validates_renames_and_reports_checksum`, `image_validate::detects_known_magic` |
| T06-08 | HTTP200 HTML / rỗng / truncated / oversized → reject | FIXTURE | PASS | `image_validate::t06_08_*`, `transfer::html_body_never_lands_as_staged_page`, `engine_tests::hostile_http200_html_page_fails_without_retry_burn` |
| T06-09 | 429/5xx/timeout → backoff hữu hạn, worker khác chạy | FIXTURE | PASS | `engine_tests::transient_failures_requeue_then_exhaust_to_failed`, `downloader::tests::backoff_grows_and_stays_bounded` |
| T06-10 | 401/403 từ chối; unsafe redirect → lỗi cuối an toàn, không retry | FIXTURE | PASS | `classify_status` 401/403→PermissionDenied(retryable=false); `http::redirect_targets_are_revalidated`; `madara::page_manifest_rejects_foreign_image_host` |
| T06-11 | Pause khi đang chạy + khi queued → DB/UI nhất | FIXTURE | PASS | `engine_tests::pause_midflight_then_resume_skips_stored_pages` (paused + completed=2), `scheduler::pause` |
| T06-12 | Resume sau pause + restart → chỉ skip trang đã verify | FIXTURE | PASS | `engine_tests` (re-fetch==2 trang thiếu), `checkpoint::t06_12_*` |
| T06-13 | Cancel khi tải → dừng an toàn, không false-complete, không xóa user file | FIXTURE | PASS | `engine_tests::cancel_before_work_stops_cleanly`, `exporter::publish_artifact` Conflict |
| T06-14 | Retry failed sau lỗi transient → transition hợp lệ, reuse trang verified | FIXTURE | PASS | `download_repo::illegal_state_transitions_are_no_ops`, `engine_tests` requeue |
| T06-15 | Crash sau page rename, trước checkpoint → reconcile không hỏng output | FIXTURE | PASS | `recovery::t06_15_crash_after_page_rename_before_checkpoint_resumes` |
| T06-16 | Crash sau CBZ rename, trước DB commit → nhận artifact, sửa idempotent | FIXTURE | PASS | `recovery::t06_16_completed_task_with_vanished_file_clears_offline_flag`, `exporter::publish_artifact` adoption |
| T06-17 | CBZ Windows: ZIP finalized, zero-padded order, count, readable | FIXTURE | PASS | `exporter::t06_17_cbz_roundtrip_ordered_verified`, `tampered_archive_fails_verification` |
| T06-18 | Image-folder output: chỉ publish folder đủ+bộ verify | FIXTURE | PASS | `exporter::t06_18_folder_only_publishes_complete`, `engine_tests::folder_output_publishes_only_verified_tree` |
| T06-19 | Trùng tên / reserved Windows / traversal / reparse → không escape, không đè | FIXTURE | PASS | `storage::t06_19_sanitizer_neutralizes_windows_hazards`, `name_components_are_enforced`, `ensure_under_root_blocks_symlink_escape`, `relpath_rejects_traversal_and_absolute` |
| T06-20 | Đĩa missing/readonly/full; DB write fail → lỗi an toàn, giữ artifact cũ | FIXTURE (một phần) | PASS (logic) | `exporter` Conflict giữ file người dùng; IO→AppError::Io; **disk-full thực tế: NOT RUN** (không giả lập OS-level) |
| T06-21 | Đóng/mở app với queued/active/paused → rehydrate theo ý, không zombie | FIXTURE | PASS | `download_repo::recovery_resets_active_states_but_keeps_user_pause`, `recovery::t06_15` |
| T06-22 | UI event race, listener teardown, source disable | FIXTURE | PASS | `phase06.test.tsx` (revision guard + unsubscribe + status), `progress::progress_frames_are_coalesced_transitions_are_not` |
| T06-23 | Chapter lớn/nhiều task → cap tài nguyên, event bounded | FIXTURE | PASS | `MAX_PAGE_COUNT`/`MAX_BATCH_SIZE`/`MAX_TRACKED_TASKS`, `progress` coalesce test |
| T06-24a | Regression Phase 01–05 (search/import/save/Dark Purple) | FIXTURE | PASS | vitest phase03/04/05 + app.test tất cả xanh (65) |
| T06-24b | **Live smoke tải thật 1 chapter → CBZ trên Windows** | LIVE | **BLOCKED / PENDING-MANUAL** | Cần user: mạng dev từng block MangaDex; truyentuoitho WAF pace + host ảnh chưa verify nằm trong allowlist exact-host. Engine sẵn sàng, **không tuyên bố live PASS**. |

## Ghi chú trung thực

- Mọi PASS ở T06-01..23 là **FIXTURE/scripted + SQLite tạm**, không chứng minh nguồn live chạy được. Đây là gate kỹ thuật engine, không phải feature live.
- **Live CBZ đầu-cuối (T06-24b): BLOCKED**, cần user chạy trên mạng không bị chặn với nguồn có ảnh cùng host allowlist. Nếu ảnh nằm host khác host site → engine chặn (đúng policy), cần xét duyệt source-policy riêng, KHÔNG mở wildcard.
- `tauri build` (MSI/NSIS) chưa chạy lại từ Phase 04 → gate trước release còn mở.
- Không có data-loss/security defect đã biết trong phạm vi fixture.
