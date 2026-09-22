# Phase 06 — Integration notes (diff thật)

**Commit base:** `0a31fa5` (Phase 05) · **Ngày:** 2026-09-22 · **Files:** 22 mới + ~29 sửa

## Backend Rust (mới)

| File | Nội dung |
|---|---|
| `src-tauri/migrations/0004_download_engine.sql` | Rebuild `download_tasks` (widen status CHECK → 10 lifecycle states, giữ `pdf` cho row cũ) + cột `attempts/next_retry_at/error_message/revision/finished_at/output_relpath`; `chapters.local_path/output_format`. Forward-only, procedure chính thức SQLite, FK off trong swap, dữ liệu 04/05 giữ nguyên (test T06-01 replay từng file migration) |
| `src-tauri/src/db/repositories/download_repo.rs` | Task/page repository: queue_batch dedup 1 transaction, transition-guarded (revision bump), claim_next (priority+retry window), ensure_page_rows (+prune manifest co lại), mark_page_stored (recompute completed_pages từ rows), requeue_with_backoff, record_failure, publish_completed (task+chapter 1 transaction), recover_stale_tasks, tasks_in_status |
| `src-tauri/src/downloader/mod.rs` | Hằng số (MAX_ATTEMPTS=3, CHAPTER_MAX_BYTES=400MB…), backoff lũy thừa jitter deterministic |
| `downloader/storage.rs` | Roots `cache/downloads/<task>` + `library/`; sanitize label Windows (reserved names, separators, ≤80, lowercase ascii); artifact name chứa task-token `-t<token>` (traceability + collision); assert_safe_name_component; ensure_under_root (canonicalize + containment) |
| `downloader/image_validate.rs` | Magic bytes PNG/JPEG/GIF/WebP; reject HTML/rỗng/truncated/oversized; extension từ type verify; CRC32 tự cài (vector test) |
| `downloader/transfer.rs` | fetch→validate→`*.part` write+flush→verify on-disk→atomic rename cùng volume; discard part files |
| `downloader/checkpoint.rs` | Reconcile DB `stored` ↔ file thật (size+CRC); reset page hỏng về pending, recompute counter |
| `downloader/exporter.rs` | ZIP STORE tự cài (basename-only, zero-padded, DOS date cố định); verify-by-name (đọc EOCD+central+CRC từng entry); publish adopt-if-verified, Conflict nếu file lạ; folder mode part-dir→rename |
| `downloader/progress.rs` | TaskSink trait + emitter: transition tức thì, progress gộp ≥250ms/task, revision trong payload |
| `downloader/worker.rs` | 1 chapter: re-check enablement + capability + manifest identity → resolving→downloading (chunk theo imageConcurrency, flag check tại boundary)→verifying (reconcile)→exporting (build+verify+publish)→completed (DB commit SAU filesystem); pause/cancel cooperative; fail_or_requeue (transient ≤3 attempts, backoff persisted) |
| `downloader/scheduler.rs` | Coordinator: dispatcher loop (wake + tick 1s), capacity = chapterConcurrency clamp 1..5, action API (pause/resume/cancel/retry/priority), shutdown cooperative, SlotGuard RAII (slot không leak khi worker panic) |
| `downloader/recovery.rs` | Startup: recover_stale_tasks (active→queued, pausing→paused), discard `.part` staging của task chưa xong, xóa phantom offline flag khi artifact biến mất; crash boundary B adopt-idempotent qua export lại |
| `downloader/engine_tests.rs` | Scripted adapter + 9 test e2e (pipeline CBZ, resume-skip-stored, pause midflight, retry budget, unsupported/hostile/disabled, folder, no-overwrite) |
| `commands/downloads.rs` | 7 command + TauriEventSink (emit `download-task-updated`); queue pre-gates capability TRƯỚC khi ghi, per-item errors; không giữ registry mutex qua await |

## Nguồn adapter (mở rộng, không song song hóa)

- `sources/adapter.rs`: `page_manifest`/`fetch_image` default-method (⇒ adapter chưa khai báo = từ chối); thêm `SourceError::PermissionDenied` (401/403, không retry).
- `sources/dto.rs`: `Capability::Pages`, `PageDescriptor`, `PageManifest` (Rust-internal, không Serialize).
- `sources/http.rs`: `Transport::get_bytes(url, max)` — cùng GuardedHttp policy (exact-host, redirect tự thẩm định, pace 3s), cap `IMAGE_MAX_BYTES=20MB`; mock mở rộng.
- `providers/madara_generic.rs`: manifest từ `.reading-content img.wp-manga-chapter-img` (fallback loose), src/data-src trim, dedup, **foreign host → fail whole manifest** (không im lặng mở allowlist).
- `providers/mangadex.rs`: **KHÔNG** implement download — at-home API trả pattern host động ⇒ xung đột exact-host; UI ẩn điều khiển tải. **Ghi chú deviations** so với brief §5: đây là blocker chính sách, không phải thiếu code.
- `providers/fixture.rs`: manifest + fetch deterministic có scenario trigger (`__net`, `__corrupt`) cho test engine.

## Frontend

- `src/types/downloads.ts` (DTO mirror + action table UI + format helpers), `src/types/backend.ts` (DownloadTaskDto mở rộng, LocalChapterDto `downloaded/outputFormat`).
- `shared/api/desktop.ts`: 7 invoke + `subscribeDownloadTask` (listen/unlisten), code `SOURCE_FORBIDDEN`.
- `services/downloads.api.ts` + `localServices.downloads` (boundary production qua `desktop.ts` duy nhất — bất biến §3.1 giữ).
- `features/downloads/DownloadsPage.tsx`: **manager thật** — snapshot đầu + event merge có revision guard, refetch sau action/focus, actions theo transition, speed chỉ khi đo, lỗi+đường dẫn verify hiển thị trung thực. Trang Demo cũ rời live view (journey demo vẫn ở sandbox Manga Detail + unit tests `mockServices.test.ts`).
- `features/library/LocalMangaPage.tsx` mới (`/library/:mangaId`): chọn 1/N/all (cap 100), format theo Settings, queue→kết quả created/reused/errors + link Downloads; nút tải ẩn khi nguồn thiếu `pages`; badge "Đã tải offline" từ cờ đã verify của Rust.
- `useDownloadQueue.ts`: subscription lifecycle + teardown, không optimistic-complete.
- Settings concurrency/output: đã thật từ Phase 04 — không đổi hợp đồng.

## Sửa ngoài brief (kê khai)

1. `vite.config.ts` watch ignore `src-tauri` — bug fix từ phiên chạy trước (EBUSY khi cargo build), đã báo user.
2. `tests/phase03.test.tsx`: gỡ 2 UI-test demo-transitions trên /downloads (hành vi demo service vẫn phủ ở `mockServices.test.ts`); `tests/app.test.tsx`: assertion header thật.
3. `manga_repo::list_chapters` +`local_path/output_format` (JOIN sẵn cột) cho badge offline.

## Deferred / follow-up (không phải gap ẩn)

- `tauri build` packaging: NOT RUN (gate trước release).
- Live end-to-end download: PENDING-MANUAL (T06-24b) — WAF/pace + xác minh host ảnh thuộc allowlist.
- Image host khác host site (phổ biến với Madara fork): nếu live fail → cần source-policy review riêng, engine đã chặn đúng.
- Nút "mở file/thư mục" trong Downloads: KHÔNG làm — cần capability `opener` mới (bất biến §3.2 cấm fs/shell; brief Phase 06 không cấp). UI hiển thị relative path đã verify thay thế.
- DB write contention dưới tải lớn: busy_timeout 5s mặc định; chưa cần tuning cho khối lượng cá nhân.
