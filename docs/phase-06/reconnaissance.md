# Phase 06 — Reconnaissance (P06-01)

**Ngày:** 2026-09-22 · **Branch:** `master` @ `0a31fa5` (Phase 05 round-2)
**Tree đầu phiên:** `M vite.config.ts` (fix watcher EBUSY khi `tauri dev` — đã báo cáo user), `M src-tauri/Cargo.toml` chỉ do CRLF, không chênh nội dung. Giữ nguyên, không reset.

## 1. Cổng tiên quyết (Phase 05)

- `docs/phase-05/self-review.md`: vòng 1+2 PASS. Bước reviewer ngoài đã được **user gỡ (2026-09-22)** theo `docs/HANDOFF.md` §5 — quy trình hiện hành: implement → self-review → gates → commit local → report.
- Gate hiện hành toàn xanh: lint · typecheck · vitest 63/63 · vite build · cargo fmt/check/clippy `-D warnings`/test 45/45 · `tauri dev` smoke exit 0 (đã kiểm chứng lại hôm nay).
- User đã chốt contract Phase 06 (HANDOFF §5.5): tải thuần ảnh chương từ nguồn đã đăng ký → `.cbz`; `get_page_manifest` nội bộ theo adapter; checkpoint qua `download_pages` (schema có từ 0002); hàng đợi qua `download_tasks`; **không đổi CSP/capability**.

## 2. Nguồn và quyền tải ảnh (chính sách đã hiệu chỉnh của chủ dự án)

Theo `docs/phase-05/source-decision.md` (user chỉ đạo 2026-09-22):

- **Madara generic** (nguồn user đăng ký, vd `truyentuoitho.com`): site cung cấp sẵn nút "Tải trọn bộ … về máy" cho người đọc; app chỉ dùng endpoint công khai frontend của site gọi; không bypass login/paywall/DRM; throttle ≥3s/request. Chương trình ảnh nằm tại `.reading-content img.wp-manga-chapter-img` (dom-hints của chính site; Phase 05 ghi chú "chỉ dùng ở Phase 06"). → **Có đủ sở cứ để bật download cho adapter Madara.** Host phục vụ ảnh: **chưa xác minh live** — policy hiện hành exact-host = host site. Nếu live ảnh nằm host khác → chặn, ghi blocker, không tự ý mở allowlist (§5 brief).
- **MangaDex**: API at-home trả về **pattern host động** (`https://{s}.provider/...`) — xung đột trực tiếp với bất biến exact-host allowlist của `GuardedHttp` (HANDOFF §3.8). Mở wildcard = "silently broaden allowlist" → cấm theo §5 brief. → **MangaDex: UNSUPPORTED_OPERATION cho download trong Phase 06** (UI phải ẩn điều khiển tải). Live-verify MangaDex vẫn BLOCKED-BY-NETWORK từ Phase 05.
- **Fixture** (`fixture-demo`, test-only, không vào registry production): sinh manifest + bytes deterministic phục vụ T06.

## 3. Schema hiện có (mapping brief → repo thật)

`0001_init.sql`: `app_settings(key,value_json,updated_at)`. `0002_domain.sql`:

| Bảng | Trạng thái cho P06 |
|---|---|
| `download_tasks` | ĐÃ có: id, chapter_id (FK RESTRICT), status CHECK **chỉ** `queued/paused/failed/completed/cancelled`, target_format CHECK `cbz/folder/pdf`, priority, completed_pages, total_pages, error_code, created/updated_at. **THIẾU** các trạng thái hoạt động resolving/downloading/verifying/exporting (+pausing), attempts/next_retry_at, error_message, revision, finished_at, output_relpath → cần migration `0004` rebuild CHECK (forward-only, giữ dữ liệu) |
| `download_pages` | ĐÃ có đủ khung checkpoint: PK (task_id,page_index), storage_key, status `pending/stored/failed`, size_bytes, checksum, FK CASCADE. Không cần sửa |
| `chapters` | Có `page_count`; **thiếu** `local_path`/`output_format` của artifact đã xác minh → ALTER ADD COLUMN (an toàn, forward-only) |
| `mangas`/`sources` | Đủ: (source_id, source_manga_id) UNIQUE; `sources.adapter/base_url` (0003) dẫn xuất adapter Madara |

`app_settings` đã có phím validation: `chapterConcurrency` 1–5, `imageConcurrency` 1–8, `outputFormat` cbz/folder/pdf — **tái sử dụng nguyên trạng**; `pdf` bị tầng download từ chối (ngoài scope). DB tại app_data_dir, WAL mặc giữ nguyên, pool 5 connections, FK pragma per-connection.

## 4. Nền Rust hiện có (điểm tái sử dụng, không tạo song song)

- `sources/adapter.rs`: trait `SourceAdapter` (info/owns_url/search/resolve_url/manga_detail) + `SourceError` taxonomy → **mở rộng** bằng method `page_manifest` + `fetch_image` mặc định trả `UnsupportedOperation`.
- `sources/http.rs`: `GuardedHttp` (https-only, exact-host, chặn IP riêng/CGNAT/multicast, redirect tự thẩm định từng hop, pace 3s/host, cap 2MB body) + `mock::MockTransport` → thêm method `get_bytes(url, max)` vào trait `Transport` (cap riêng cho ảnh, không nhân client thứ hai).
- `db/repositories/task_repo.rs`: đang read-only — mở rộng ghi có kiểm soát. `manga_repo` có `ensure_source/upsert_manga/upsert_chapter`.
- `app_state.rs` `AppState { db, registry }` → thêm `downloads: DownloadCoordinator` (state máy + worker handles).
- `commands/*`: pattern `#[tauri::command] async fn ...(State<AppState>) -> Result<T, AppError>`; AppError wire `{code,message,retryable}` snake UPPER, message tiếng Việt, không path/SQL/stack.
- **Chưa có** dependency ZIP/stream → thêm `zip` (write Stored + đọc lại để verify) và tokio features `sync` — tối thiểu, có mục đích rõ.
- `services/storage_service.rs`: đã tạo `app_data/covers|cache` — staging thuộc `app_data/cache/downloads/<task-id>/`, thư viện xuất `app_data/library/<manga>/<chapter>.cbz` (quyền chọn root khác: ngoài scope phase này, ghi rõ).

## 5. Frontend hiện có

- Mọi invoke đi qua `src/shared/api/desktop.ts` (bất biến §3.1). Đã có `listLocalDownloadTasks`.
- `src/domain/download.ts` Demo state machine 5 trạng thái — Journey Demo giữ nguyên nhãn; production dùng DTO mới `src/types/downloads.ts`.
- `DownloadsPage` + `DownloadCard` (Dark Purple) đang mock; `MangaDetailPage` + `ChapterSelectRow` đã có select chương (demo queue) → điểm nối batch submit.
- Provider chuyển mạch mock/backend: `src/services/provider.ts` + `local-library.api.ts` pattern — làm tương tự cho `downloads.api.ts`.
- Test harness vitest 63 pass; pattern `tests/phaseNN.test.tsx`.

## 6. Kết luận cổng

- Engine + fixture + Madara path: **GO**.
- Live end-to-end download: **PENDING-MANUAL** (WAF site, cần user bấm thử như Phase 05) — không tuyên bố ACCEPT live nếu chưa chạy; MangaDex download: **BLOCKED by policy** (ghi rõ, không phải lỗi code).
