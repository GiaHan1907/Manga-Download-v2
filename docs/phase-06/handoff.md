# Phase 06 — Handoff & Phase 07 contract

**Branch:** `master` (local commit, chưa push) · **Ngày:** 2026-09-22 · **Cổng:** engine fixture-green; live download PENDING-MANUAL.

## 1. Tái lập & review

```bash
# Frontend gates
npm ci && npm run typecheck && npm run lint && npm run test -- --run && npm run build
# Backend gates (cần Rust stable-msvc + VS Build Tools C++)
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
# Desktop smoke
npm run tauri dev
```
Kết quả mong đợi: **65 vitest / 84 cargo test / clippy 0 warning**. Mọi test dùng SQLite tạm + scripted adapter (không network).

## 2. Cái đã thật / chưa thật (đọc trước khi test tay)

- **THẬT:** hàng đợi tải (Downloads), chọn chương + xếp hàng (Library → mở 1 truyện đã lưu → `/library/:id`), checkpoint trang, pause/resume/cancel/retry/priority, CBZ/folder trên đĩa, cờ offline, event tiến độ — tất cả Rust + SQLite.
- **CHƯA/ bị chặn:**
  - Tải **live** từ Madara source user: cần bấm thử trên máy user; nếu ảnh nằm host khác host site ⇒ engine CHẶN (`SOURCE_*`/validation) theo policy — đó là lúc cần source-policy review, **không** được nới allowlist.
  - MangaDex: không có capability `pages` → UI ẩn nút tải. Đúng thiết kế.
  - Nút "mở file/thư mục": chưa có (cần capability `opener`; bất biến hiện hành cấm fs/shell từ renderer).
  - `npm run tauri build` (MSI/NSIS): **NOT RUN** từ sau Phase 04 → gate mở trước release.
  - App-shutdown hook `Coordinator::shutdown()` đã có nhưng **chưa** gắn vào `RunEvent::ExitRequested`; an toàn vì restart-recovery đã kiểm chứng (T06-21).

## 3. Cách test live thủ công (đề xuất)

1. Settings → Sources → thêm nguồn Madara HTTPS hợp lệ → bật.
2. Discover (tab live) → tìm → mở preview → **Save metadata**.
3. Library → mở truyện vừa lưu (`/library/:id`) → chọn chương → **Tải N chương**.
4. Downloads → quan sát resolving→downloading (trang/bytes/speed)→verifying→exporting→completed; thử Pause/Resume giữa chừng; tắt/mở app để xem rehydrate.
5. Kiểm file `.cbz` trong `<app_data>/library/<title-id>/` (mở bằng 7-Zip: entry `000000.png`…).

## 4. Contract cho Phase 07 (Reader)

Reader **chỉ** nhận metadata chương đã kiểm chứng qua API sẵn có:
- `app_get_local_manga(mangaId)` → `LocalChapterDto { id, localPath?, outputFormat: "cbz"|"folder"|null, downloaded }`. `downloaded=true` ⇔ artifact đã verify tồn tại (startup recovery dọn cờ giả).
- Đường dẫn resolve **bên Rust** qua `storage::library_root(app_data) + output_relpath` (relative, đã validate `validate_relpath`). **Không** để Reader parse `*.part`, **không** fetch URL nguồn, **không** nhận path từ renderer.
- Định dạng: CBZ = ZIP **Stored**, entry basename-only zero-padded `000000.<ext>`, ext theo loại ảnh đã verify; hoặc thư mục ảnh cùng thứ tự. Bảo đảm toàn vẹn: mỗi entry khớp CRC32 checkpoint.
- Refresh Library sau khi 1 task `completed`: refetch `app_get_local_manga` (đừng tin cờ trong bộ nhớ cũ). Event `download-task-updated` status `completed` là tín hiệu refresh.
- ID ổn định (manga/chapter UUID) giữ nguyên; reading progress không đổi khi tải xong.
- Error taxonomy & root resolution: xem `docs/IPC_CONTRACT.md` + `docs/phase-06/download-design.md` §5,§7.

## 5. Known limitations (không ngụy trang)

- Resume = page-level (bỏ trang đã verify), **không** byte-range HTTP; `.part` dở bị loại bỏ và tải lại trang đó.
- Concurrency mặc định nhỏ (chapter 1–5, image 1–8) + pace ≥3s/host; đủ lịch sự, chưa tối ưu throughput.
- `output_relpath` trỏ vào app-managed library root; đổi root cho task cũ không tự migrate (theo thiết kế).
- Test disk-full/readonly drive cấp OS: **NOT RUN** (logic lỗi IO đã có đường dẫn an toàn + test ở mức file op).
- Live CBZ thật: **chưa** có bằng chứng đầu-cuối trên nguồn thật.

## 6. Quyết định

- Không push/merge/deploy (theo `AGENTS.md` §9 + yêu cầu). Bàn giao cho user/reviewer.
- Phase 07 chỉ bắt đầu khi: (a) engine fixture-green (đã đạt), và (b) user xác nhận live-download hoặc chấp nhận build fixture-only cho Reader thử nghiệm.
