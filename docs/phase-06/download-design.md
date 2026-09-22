# Phase 06 — Download design (P06-01)

Rust là thẩm quyền duy nhất (renderer chỉ gửi chapterId + hành động). Một SQLite pool. Không IPC chung cho URL/header/path.

## 1. State machine (persisted, thực thi trong `download_service.rs`)

```
queued → resolving → downloading → verifying → exporting → completed
   ↑          |             |            |           |
   └──── retry (từ failed) / automatic backoff-requeue ────┘
queued/downloading/… → pausing → paused      (pause hợp tác, chỉ acknowledged khi writes đã ổn định)
bất kỳ non-terminal  → cancelled              (không xóa file người dùng đã có)
failed               ← lỗi không recover được hoặc hết retry budget
```

- `completed` **chỉ khi**: artifact final đã verify tồn tại + DB commit thành công.
- Crash recovery: task ở trạng thái hoạt động (`resolving/downloading/verifying/exporting/pausing`) → về `queued` (hoặc `paused` nếu intent cuối là pause); không bao giờ hiển thị `downloading` khi không có worker sống.
- `download_pages`: `pending → stored → (failed)`; checkpoint ghi **sau** khi file staged đã validate + rename; `completed_pages` luôn recomputed từ COUNT(rows stored), không tin counter cũ.
- Transition table Rust-only, mỗi transition là 1 transaction ngắn (không giữ transaction qua network IO).

## 2. Migration `0004_download_engine.sql` (forward-only)

1. Rebuild `download_tasks` (procedure chính thức của SQLite, `PRAGMA foreign_keys=OFF` trong migration, dữ liệu 04/05 giữ nguyên):
   - status CHECK mở rộng: `queued,resolving,downloading,pausing,paused,verifying,exporting,completed,failed,cancelled`
   - thêm cột: `attempts INTEGER NOT NULL DEFAULT 0`, `next_retry_at TEXT`, `error_message TEXT`, `revision INTEGER NOT NULL DEFAULT 0`, `finished_at TEXT`, `output_relpath TEXT`
   - giữ index `idx_tasks_status_created`, `idx_tasks_chapter`; thêm `idx_tasks_priority` (`ORDER BY priority DESC, created_at`).
2. `ALTER TABLE chapters ADD COLUMN local_path TEXT; ALTER TABLE chapters ADD COLUMN output_format TEXT;`
3. Test nâng cấp: DB Phase 05 thật → apply → catalog/favorites/progress còn nguyên (T06-01).

## 3. Hợp đồng adapter → engine

```rust
pub struct PageDescriptor { pub index: usize, pub url: String }        // url KHÔNG bao giờ ra UI
pub struct PageManifest { pub source_chapter_id: String, pub pages: Vec<PageDescriptor> }
// trait SourceAdapter, default = Err(UnsupportedOperation):
async fn page_manifest(&self, source_manga_id:&str, source_chapter_id:&str) -> Result<PageManifest, SourceError>;
async fn fetch_image(&self, url:&str) -> Result<Vec<u8>, SourceError>; // qua Transport.get_bytes(url, IMAGE_MAX)
```

- `Capability::Pages` mới → UI chỉ hiện nút tải khi adapter khai báo (MangaDex: không khai báo — at-home cần wildcard host, xung đột exact-host; ghi blocker).
- **Madara**: parse chapter HTML `.reading-content img.wp-manga-chapter-img` (`src`/`data-src`, trim khoảng trắng) — thứ tự DOM = thứ tự trang; validate mỗi URL: https, exact-host = host site, slug charset; cap 300 trang; tối thiểu 1. Khi manifest đổi (re-resolve), reconcile checkpoint theo **page index + size/hash** — không khớp thì re-download, không ghép lạ.
- **Fixture**: manifest deterministic `{id}-c1` → 3 trang, `{id}-c2` → 2 trang; `fetch_image` trả bytes nhúng sẵn theo URL (không network).
- Validate ảnh (engine, không tin adapter): magic bytes PNG/JPEG/GIF/WebP; reject HTML `<`, thân rỗng, quá `IMAGE_MAX_BYTES=20MB`, quá tổng chương `CHAPTER_MAX_BYTES=400MB`; extension suy từ type đã verify.

## 4. Scheduler & worker (`downloader/`)

- `DownloadCoordinator` (managed state): mpsc wake-signal + `set<task_id>`; vòng lặp chọn task theo `priority DESC, created_at` khi còn permit; permit = `Semaphore(chapterConcurrency)` đọc từ settings lúc dispatch.
- Worker 1 chapter: `resolving` → manifest → tạo page rows (transaction) → `downloading`: mỗi trang fetch qua `Semaphore(imageConcurrency)` + `GuardedHttp` pace 3s/host → ghi `.part` → fsync/close → validate → rename → checkpoint row `stored` → `verifying`: đếm lại rows+files → `exporting` → `completed`/`failed(attempts++)`.
- Retry: chỉ `Network/Upstream/RateLimited/timeout`; backoff lũy thừa có jitter (cap 15 phút), `max_attempts=3` persist `next_retry_at`; `NotFound/Parse/UnsupportedOperation/redirect-unsafe/disk-full` → failed ngay. `429 Retry-After` tôn trọng nếu ≤ ngân sách.
- Pause/cancel: `Arc<AtomicBool>` flag trong handle; worker kiểm tra tại ranh giới trang (quiesced writes) rồi service commit `paused`/`cancelled`; xóa staging chỉ sau cancel và chỉ folder của task. Cancel không đụng artifact đã publish.
- Shutdown app: signal cho mọi worker, chờ 5s, DB giữ nguyên trạng thái đã checkpoint (không mark gì giả).
- Tốc độ/bytes: đo trong memory (window 3s), không persist; event mang số liệu chỉ khi đang đo.

## 5. File layout & export (atomic)

```
<app_data>/cache/downloads/<task-id>/000001.jpg.part → 000001.jpg   (staging, app-owned)
<app_data>/library/<manga-slug>-<manga-id>/ch-<chapter-id>-t<task-id>.cbz|.folder/
```

- Tên file final chứa **task id** → recovery xác định được artifact của task nào; collision = hoặc artifact của chính task (verify & nhận), hoặc fail `OUTPUT_EXISTS` — không bao giờ đè.
- CBZ: ghi `*.cbz.part` **trong cùng thư mục đích** (same volume), ZIP **Stored** (không re-encode), entry basename-only `000001.jpg` (chống ZIP slip), đóng central directory → **đọc lại verify**: đủ số entry, đúng thứ tự, CRC từng entry → rename atomic → commit `chapters.local_path/output_format` + task `completed` (1 transaction; nếu commit fail, lần khởi động tới nhận diện artifact đã verify và repair idempotent).
- Folder mode: copy bộ trang đã verify vào `*.part-folder/`, verify đủ+N+order, rename → commit.
- Path safety: mọi đường dẫn nội bộ Rust dựng từ ID đã validate `[a-z0-9._-]`, sanitize tiêu đề (bỏ `..`, separators, device name Windows, control chars, chấm/cách cuối, reserved names, giới hạn 80 ký tự); canonicalize rồi assert nằm dưới root đã duyệt; từ chối symlink/reparse escape.
- Recovery khởi động (chỉ quét thư mục task được DB biết, có bound): xóa `*.part` (staging + library), verify artifact final đã tồn tại mà DB chưa commit → nhận về; thiếu file của row `stored` → reset trang đó về pending; không bao giờ quét đệ quy thư viện người dùng.

## 6. IPC (tên theo contract hiện hành, camelCase DTO)

| Command | Vào | Ra |
|---|---|---|
| `queue_chapter_downloads` | `chapterIds: string[] (≤100), format?: 'cbz'\|'folder'` | `QueueResultDto { tasks: DownloadTaskDto[], reused: string[], errors: {chapterId, code, message}[] }` |
| `get_download_task` | `taskId` | `DownloadTaskDto` |
| `pause_download` / `resume_download` / `cancel_download` / `retry_download` | `taskId` | `DownloadTaskDto` (state sau acknowledge; từ chối transition sai → `INVALID_STATE_TRANSITION`) |
| `set_download_priority` | `taskId, priority(-10..10)` | `DownloadTaskDto` |
| `app_list_local_download_tasks` (REUSE + mở rộng) | status/limit/offset | `PagedResult<DownloadTaskDto+>` thêm `errorMessage,nextRetryAt,finishedAt,outputRelpath,revision,downloadedBytes,speedBytesPerSecond` |

Dedup: submission trùng chapter đang có task **non-terminal** → trả reused id (không tạo task 2, không đè artifact completed). Event `download-task-updated` payload `{ taskId, revision, snapshot }`: hợp nhất ≥250ms cho progress, **tức thì** cho mọi chuyển trạng thái; UI giữ revision cao nhất, refetch sau action/focus.

## 7. Frontend

- `src/types/downloads.ts` (DTO khớp Serde), `src/services/downloads.api.ts` (wrapper desktop + demo fallback theo pattern local-library.api.ts), hàm invoke mới trong `desktop.ts`.
- `MangaDetailPage` production tab: checkbox chương → "Tải N chương (CBZ)" → kết quả queued + link Downloads. Nút ẩn khi nguồn không có `pages` capability.
- `DownloadsPage` real: snapshot từ SQLite + subscribe event; action buttons chỉ theo transition hợp lệ; mở file chỉ với artifact verified; dữ liệu demo phải có nhãn Demo rõ.
- `SettingsPage`: chapterConcurrency/imageConcurrency/outputFormat đã có — nối vào backend settings thật (giữ validation 1–5/1–8).
- `LibraryPage`: huy hiệu "Đã tải" chỉ khi `local_path != null` và Rust đã verify (flag từ backend, không suy luận ở UI).

## 8. Cổng trung thực

- Fixtures PASS ≠ live PASS. Live smoke trên Madara source của user: **PENDING-MANUAL** (cần user bấm trên mạng thật, WAF pace). Nếu ảnh nằm host ngoài allowlist → tính năng live ghi **BLOCKED**, engine vẫn bàn giao fixture-tested.
