# Kế hoạch cải thiện Tính năng — Feature Roadmap v1.1+

> **Trạng thái (2026-09-23):** **F2 + F5 + F1 + F4 + F3a + F3b ĐÃ TRIỂN KHAI** — tổng 12 lệnh IPC mới (registry 50; kèm lệnh phụ trợ `queue_chapter_download_by_url` — dán link chương tải thẳng, sinh sau live probe); dependency `flate2` + `tauri-plugin-dialog` đã user-duyệt. F3a: `cbz::open` đọc cả STORE + DEFLATE (guard cap khi mở + chặn `total_out` phình quá khai báo → chống zip-bomb; method lạ ⇒ CBZ_INVALID); reader app + reconcile dùng chung nên CBZ deflate ngoài chạy suốt. F3b: `library_import_cbz` mở dialog native **từ Rust** (renderer không gửi/nhận path), parse CBZ untrusted, chép vào library root dưới tên sinh-từ-UUID đã sanitize, ghi `chapters` + `download_tasks` 'completed' trong MỘT tx → Library/Reader/reconcile đối xử y hệt chương tải thật; KHÔNG thêm capability `dialog:`. F1 về sớm hơn lịch v1.1: watcher ~6h qua `manga_detail` adapter (pacing/GuardedHttp giữ nguyên), schema 6 `chapter_alerts` với baseline lần đầu (không spam toàn bộ lịch sử truyện), opt-in per-manga + khóa chung `settings.watchEnabled`, notification tổng + event `library-alerts`, filter "Có chương mới" + badge, banner chi tiết truyện + nút "Kiểm tra chương mới" — tick cưỡng ép kiểm tra ngay (bỏ cửa sổ due; loop nền vẫn tôn trọng backoff), rate-limit 60s CAS → `DB_CONFLICT` (review vòng 1 sửa: trước đó manual tick vẫn bị due-window chặn nên thường check 0 truyện). F4 (lịch sử đọc — schema 7 `read_history`, UPSERT trong transaction save_progress khi đến 2 trang cuối, list/xóa trong LocalMangaPage) cũng xong, sớm hơn lịch v1.2. Còn lại: **F6** owner-gate. Gates F3: vitest 99/99 · cargo 155/155 · clippy sạch · `tauri dev` smoke (dialog import chạy từ Rust, không lộ path). Sau đó: live probe R-1 (user-chỉ đạo) tải thật Ninja Loạn Thị ch.1 từ truyentuoitho.com (188/188 trang → CBZ verify) — phát sinh feature multi-host nguồn (`imageHosts`, migration 8) + AJAX `ajax/chapters/`, đã đóng R-1 ở mức headless.
> **Loại:** plan tính năng (khác `UI_IMPROVEMENT_PLAN.md` — tài liệu đó thuần presentation)
> **Tác giả:** Codex Planner · **Ngày:** 2026-09-23
> **Baseline:** `master` @ `3a0c255` (sau review toàn project 4 vòng; `v1.0.0` đang `RC_TESTED — PUBLICATION BLOCKED`)
> **Quy ước:** mỗi F = một phase nhỏ độc lập, quy trình "implement → review N vòng → gates → commit local; push khi user yêu cầu".

---

## 0. Điều kiện tiên quyết (không phải việc của AI)

Toàn bộ roadmap này chỉ có nghĩa sau khi `v1.0.0` đóng được 2 blocker phía user:
1. Live download journey R-1 (1 chapter thật → CBZ → Reader) — xem `docs/HANDOFF.md` §4.
2. VM/profile sạch chạy 15 mục `docs/phase-10/WINDOWS_INSTALL_UPGRADE_TEST.md` + Reviewer ACCEPT.

Nếu R-1 fail → ưu tiên tối thượng đảo về sửa engine theo bằng chứng live, mọi F dưới đây xếp sau.

---

## 1. Bảng tổng quan

| # | Tính năng | Giá trị | Phụ thuộc mới | Migration | Rủi ro | Thứ tự |
|---|---|---|---|---|---|---|
| F1 | **New-chapter watch** (truyện có chương mới → notify + badge) | Cao nhất — lý do quay lại app mỗi ngày | Không | `0006_watch.sql` | Trung (pacing/lịch quét) | 1 |
| F2 | **Xóa lịch sử download** (clear finished/failed history) | Trung — debt có chủ đích từ P08 | Không | Không | Thấp | 2 |
| F3 | **Deflate CBZ + import file CBZ ngoài vào Library** | Cao cho offline-first | `flate` (Rust, pure); dialog phía Rust | Không | Trung | 3 |
| F4 | **Read history + screen "Đã đọc"** | Trung | Không | `0007_history.sql` (gộp được với 0006 nếu làm cùng lúc) | Thấp | 4 |
| F5 | **Queue bulk ops**: retry-all-failed, filter/search history, per-manga cancel-all | Trung | Không | Không | Thấp | 5 |
| F6 | **Updater production (branch A P10)** | Chỉ khi phân phối công khai | cert + endpoint + key custody (chỉ user cung cấp) | Không | Cao nếu tự chế | CŔA QUYẾT ĐỊNH OWNER |

**Non-goals toàn roadmap (nói không trước để khỏi scope-creep):**
- KHÔNG thêm source adapter mới/theo domain cụ thể — policy hiện tại: Madara generic do user tự đăng ký; mỗi site hardcode là một cuộc chiến bảo trì.
- KHÔNG tự động hóa vượt paywall/DRF/rate-limit của site nguồn; không đổi floor pacing 3s/host (đây là safety policy, không phải setting).
- KHÔNG đổi CSP/capabilities trừ mục có ghi rõ gate ở dưới.
- KHÔNG cloud sync/account — local-first là product identity.

---

## 2. F1 — New-chapter watch ⭐

### 2.1 Người dùng nhận gì
Truyện đang lưu (mặc định chỉ favorites; per-manga override được) sẽ được app kiểm tra chương mới sau mỗi khoảng thời gian; có chương mới → notification Windows (hạ tầng đã có) + badge "Mới" ở Library + nút "Tải tất cả chương mới" ở detail.

### 2.2 Thiết kế
- **Schema `0006_watch.sql`** (forward-only, append-only per policy):
  ```sql
  ALTER TABLE mangas ADD COLUMN watch_enabled INTEGER CHECK (watch_enabled IN (0,1)); -- NULL = theo mặc định toàn cục
  ALTER TABLE mangas ADD COLUMN last_seen_chapter_key TEXT;   -- source_chapter_id "mới nhất" lần quét gần cùng
  ALTER TABLE mangas ADD COLUMN watched_at TEXT;              -- timestamp quét gần nhất (thành công THÌ mới stamp)
  CREATE TABLE chapter_alerts (manga_id TEXT NOT NULL REFERENCES mangas(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    detected_at TEXT NOT NULL, ack_at TEXT, PRIMARY KEY (manga_id, chapter_id));
  ```
  (`upsert_chapter` trong `manga_repo.rs` đã có cơ chế insert chapter mới từ detail — alerts populated trong cùng transaction quét khi `created=true`.)
- **Runner**: task nền trong `lib.rs` setup — `tokio` interval qua `tauri::async_runtime::spawn`, KHÔNG nhét vào `scheduler::Coordinator` (coordinator sở hữu download lifecycle; watch là metadata poll — tách file `src/services/watch_service.rs` + `src/downloader` không đổi).
  - Tick 6h (hard const kỳ đầu; settings key sau này), jitter ±15% theo manga_id hash (deterministic, không `rand`), ONLY khi online — probe bằng chính GuardedHttp lỗi, im lặng fail.
  - Mỗi tick chọn ≤ 20 manga đủ điều kiện (watch on, tới hạn, ưu tiên favorite rồi `last_read_at` desc); per-source: đã có ≥3s/host pacing trong `GuardedHttp` nên không thể floods; thêm cap 40 requests/tick.
  - Gọi `adapter.manga_detail(source_manga_id)` (capability `Details` đã có cho MangaDex + Madara) → diff chapter list với `last_seen_chapter_key`; chương mới đi qua `upsert_chapter` → insert `chapter_alerts`.
  - Notification: reuse plugin đã init (`lib.rs:26`) — một thông báo gộp/tick ("3 truyện có chương mới") click → mở `/library` qua event pattern `tray-open-downloads` (thêm `tray/watch-open` cùng kiến trúc bridge, capabilities KHÔNG đổi vì không permission mới).
- **IPC mới (4 lệnh, cùng quy ước validated-ID):**
  `library_set_watch { mangaId, enabled: bool|null }`, `library_list_alerts { limit, offset }`, `library_ack_alerts { mangaId }`, `watch_run_now {}` (manual check-on-demand, rate-limited: nếu tick <30s trước → `CONFLICT`).
- **Settings**: key mới `watchEnabled` (default `true`), `watchIntervalHours` fix const 6 kỳ đầu — KHÔNG cho user chỉnh trong F1 (tránh lịch hung hăng); đưa vào `AppSettings` blob (`domain/settings.rs`) với mirror `RuntimeFlags` như pattern 2 key Windows hiện tại.
- **Frontend**: LibraryPage filter "Có chương mới" (domain bump `library`); LocalMangaPage banner "+N chương mới [Tải tất cả]" → `queue_chapter_downloads(alert chapterIds)` (reuse IPC có sẵn, ≤100); badge trên MangaCard từ `chapter_alerts` count trong aggregate `list_mangas` (thêm EXISTS-subquery vào query 1-con-pass hiện hữu — perf test tripwire T09 tương tự cần chạy lại).

### 2.3 Edge cases & rủi ro
- Nguồn đổi thứ tự/không stable list → so theo `source_chapter_id` + `number` (đã persist), KHÔNG so index; `number NULL` → chỉ đánh dấu "mới" lần đầu thấy.
- App not running = không watch (không scheduled-task Windows — out of scope, không persistence ngoài process theo P10 rule §4.5).
- Truyện source chết/410 → `watched_at` không stamp, backoff nhân đôi cho manga đó (max 7 ngày), không notify lỗi.
- Privacy: chỉ metadata chapter id/title, không URL raw ra khỏi Rust.

### 2.4 Acceptance
Test Rust: fake adapter Details diff → alerts rows + notify sink call (CollectingSink pattern có sẵn); tick selection cap; backoff; ack idempotent. IPC contract update `docs/IPC_CONTRACT.md`. Vitest: filter/banner/queue-from-alert wiring với mock invoke. Gate: `cargo test`, clippy, perf tripwire `library_list` với 20 cột alerts không quá ngưỡng T09 (9.7ms baseline → ngưỡng mới ≤15ms).

---

## 3. F2 — Clear download history

Debt được ghi nhận tường minh P08 ("NOT IMPLEMENTED có chủ đích") — lý do trì hoãn nay không còn: semantics đã chắc.
- `download_clear_history { olderThanDays?: i64, statuses?: ["completed","failed","cancelled"] }` — transaction: xóa task theo filter CHỈ khi (a) task terminal, (b) artifact không tồn tại trên đĩa hoặc đã re-verify; completed-task-còn-file → giữ, trả per-item `skipped[]` (đừng tạo ma cô mồ côi — `output_relpath` + chapters).
- Xóa theo thứ tự FK: `download_pages` (cascade tự) → `download_tasks`; KHÔNG đụng `chapters.local_path`.
- UI: nút "Dọn lịch sử" trong Downloads + dialog confirm (reuse Dialog focus-trap) với checkbox "chỉ xóa mục lỗi".
- Docs: sửa `docs/phase-08/settings-and-storage-policy.md` dòng deferred + `KNOWN_ISSUES` nếu liên quan.

## 4. F3 — Deflate CBZ + Import CBZ ngoài

Hai tầng, tách được:
- **F3a Deflate reader** (pure-Rust `flate2` — dependency Rust mới, cần user duyệt dependency theo AGENTS.md): `reader/cbz.rs` thêm path inflate cho entry method=8 với guard: declared-size cap 20MB/trang như STORE, `total_out` bị chặn trong khi inflate (không zip-bomb); entry không phải ảnh vẫn skip. Mở khả năng đọc CBZ tải từ nơi khác.
- **F3b Import**: command `library_import_cbz {}` mở native dialog **từ Rust** (`tauri-plugin-dialog` Rust API — renderer KHÔNG bao giờ nhận path; plugin JS API bị loại vì lý do này) → validate ZIP + sniff manga/chapter từ `ComicInfo.xml` (tuỳ chọn) hoặc tối giản: người dùng chọn manga có sẵn trong app + đánh số chapter → copy file vào `library/<manga-dir>/` bằng tên chuẩn exporter, ghi `chapters.local_path` + `download_tasks` dòng 'completed' nhân danh import (để Library/Reader/reconcile hiện hành hoạt động không đổi một byte).
- **Gate capability:** thêm plugin dialog = cân nhắc — nếu chọn Rust-side command-only, KHÔNG cần capability mới cho webview (permission `dialog:default` chỉ cần nếu gọi từ JS — ta không gọi). Ghi rõ trong PR.

## 5. F4 — Read history

- `0007_history.sql`: `read_history(manga_id, chapter_id, read_at)` append, index `(manga_id, read_at)`; writer: `app_save_reading_progress` insert thêm dòng khi lần đầu hoàn chapter (`pageIndex ≥ pageTotal-2`) — chống spam bằng UNIQUE(manga_id, chapter_id) + UPSERT `read_at`.
- Screen: Library tab mới "Đã đọc gần đây" (query `list_mangas` sort `last_read` đã tồn tại → mở rộng sang detail màn: chương nào, khi nào).
- Kèm "Xóa history của manga" (user-chủ-đích data removal — xóa row, không đụng file).

## 6. F5 — Queue bulk ops

`retry_failed_all` (scan status='failed' + `retry_failed` từng mục trong 1 transaction-batch, cap 200), `cancel_for_manga { mangaId }` (terminal chỉ task active/cancelled of chapters belonging), UI selection theo group filter. Thuần `download_repo` + 2 lệnh IPC; làm sau F1-F2 vì giá trị thấp hơn và đụng engine (cần giữ 4 vòng test semantics pause/cancel).

## 7. F6 — Updater production (chỉ khi owner chọn)

Điều kiện cứng (từ P10 branch A): certificate Authenticode + updater signing key custody + HTTPS endpoint + VM test N→N+1. AI không tạo thay; plan chi tiết sẽ viết khi owner xác nhận kênh phân phối. Trước đó mọi release = manual download installer + hash verify.

---

## 8. Lịch trình khuyến nghị

```
v1.0.x   : F2 + F5  (no-risk cleanups, giữ nguyên contract surface nhỏ)
v1.1     : F1 (watch)  ← bản "đáng upgrade" đầu tiên
v1.2     : F3a + F4    (F3b sau khi dependency dialog được duyệt)
owner-gate: F6
```
Mỗi F: design-doc-1-trang trong `docs/phase-XX/` nếu phát sinh IPC mới → implement → review N vòng (gates đầy đủ mỗi vòng) → cập nhật `IPC_CONTRACT.md`, `HANDOFF.md` §1, README §Gates → commit local → push khi user yêu cầu.
