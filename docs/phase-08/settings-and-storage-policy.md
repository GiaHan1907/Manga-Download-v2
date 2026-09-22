# Phase 08 — Settings & Storage Policy

## 1. Settings keys (blob `settings.v1` trong `app_settings`, validate 100% phía Rust)

| Key | Loại / Range | Default | Consumer thật | Hiệu lực |
|---|---|---|---|---|
| defaultReaderMode | single/double/vertical | single | ReadPage seed khi mở chương | Lần mở chương kế tiếp |
| defaultReadingDirection | ltr/rtl | rtl | ReadPage seed | Lần mở chương kế tiếp |
| chapterConcurrency | int 1..=5 | 2 | `scheduler` capacity mỗi nhịp dispatch | Job mới ngay; job đang chạy giữ nguyên slot |
| imageConcurrency | int 1..=8 | 4 | worker per-image | Page đang bay giữ nguyên batch |
| outputFormat | cbz/folder (pdf còn được validate để tương thích DB cũ nhưng UI ẩn; queue pdf → `VALIDATION_ERROR`) | cbz | default khi xếp hàng | Task chưa tạo |
| notificationsEnabled | bool (MỚI P08) | true | `TauriEventSink::publish` — notify khi task về `completed`/`failed` | Ngay (RuntimeFlags sync khi save) |
| closeToTray | bool (MỚI P08) | false | `on_window_event` CloseRequested → hide thay vì thoát | Ngay (RuntimeFlags sync khi save) |

Quy tắc:

- Không tạo store settings thứ hai. Frontend `AppSettings`/`AppSettingsDto` mirror đúng 7 key; `deny_unknown_fields` vẫn chặn key lạ.
- `RuntimeFlags` (AtomicBool) là bản sao read-sync cho các đường KHÔNG await được (window event, event sink); command `app_update_settings`/`app_reset_settings` cập nhật flag SAU khi persist thành công → flag không bao giờ đi trước DB.
- Settings sai vùng → `VALIDATION_ERROR`, giá trị lưu không đổi (test `t04_08`).

## 2. Windows behavior policy

| Hành động | closeToTray=false (mặc định) | closeToTray=true |
|---|---|---|
| Bấm X | Exit → `ExitRequested` → shutdown coordinator ≤5s (worker cancel) → thoát process; task active về `queued` ở lần chạy sau (recovery) | Ẩn cửa sổ; process + download tiếp chạy; tray là đường quay lại duy nhất |
| Tray Show/Hide | Luôn có sẵn (icon từ default window icon) | — |
| Tray Open Downloads | Focus cửa sổ + emit `tray-open-downloads` → bridge điều hướng `/downloads` | — |
| Tray Quit | `app.exit(0)` → Đi qua cùng lifecycle `ExitRequested` (shutdown sạch). Không có tuyên bố "tải tiếp sau khi process thoát" — kiến trúc này không có background service | |

- Notification: theo CHAPTER (task completed/failed), không theo ảnh; body chỉ chứa title đã sanitize từ DB, không path/URL/header. Windows từ chối/bật Do Not Disturb → plugin fail im lặng (không crash app).
- Capability KHÔNG đổi: notification gọi từ Rust nên không cần permission IPC; `capabilities/default.json` vẫn `core:default` + 5 window perms.

## 3. Storage policy

- Root duy nhất: `library_root = <app_data>/library`, `staging_root = <app_data>/cache/downloads/<task-id>`, `covers = <app_data>/covers`. Không command nào nhận đường dẫn từ renderer.
- **Đổi library root: KHÔNG khả dụng ở Phase 08** — chưa có quy trình migrate file được review; policy cũ "new downloads only" cũng đòi root mới hợp lệ, tránh orphan file. UI ghi rõ lý do.
- `storage_status`: library_bytes = SUM(`chapters.local_size_bytes` của chapter có artifact); cache_bytes = walk `cache/` + `covers/` (bỏ qua symlink, chặn 10k entries); `root_available` = đọc được thư mục library; luôn khai báo `approximate: true`. Không trả absolute path.
- `clear_regenerable_cache`: XÓA duy nhất các thư mục staging con trực tiếp mà (a) tên khớp shape task-id app-sinh và (b) task không còn trong DB HOẶC đã `completed`/`cancelled`; cộng thêm nội dung `covers/`. Không bao giờ xóa: artifact library, `download_tasks/download_pages` (checkpoint/lịch sử), staging của task đang/ sẽ chạy tiếp. Idempotent (test `t08_clear_cache_keeps_active_and_removes_terminal_orphans`). Chạy tự động ở startup sau recovery (log số liệu nếu có xóa).
- Disk missing/full/permission: writer/transfer fail → task `failed` retryable với error an toàn; reader/reconcile báo `LOCAL_FILE_MISSING`; không có đường nào đánh dấu Completed khi file chưa verify (bất biến Phase 06 giữ nguyên).
- "Clear history" download: **không triển khai** — `download_tasks` là nguồn `has_missing` của Library và checkpoint ngữ nghĩa; nút nửa vời vi phạm DoD "mọi control phải có tác dụng thật". Đã ghi vào handoff như deferred có lý do.

## 4. Regression

- `t08_clear_cache_keeps_active_and_removes_terminal_orphans`, `t08_status_reports_db_sizes_and_root_flag`, `t08_stats_and_progress_fallback_when_chapter_not_downloaded` (Rust).
- `phase08.test.tsx`: invalidation map/store, palette (offline-only, keyboard, XSS-inert), favorites pagination, storage panel calls (vitest).
- Phases 06/07 regression: toàn bộ suite cũ vẫn chạy (downloads pagination chỉ mở rộng payload `total`).
