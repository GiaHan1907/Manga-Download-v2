# HANDOFF — trạng thái dự án cho AI/dev tiếp theo

**Cập nhật:** 2026-09-22 · **Branch:** `master` (local history, chưa từng force-push)
**Đọc trước khi làm việc:** `AGENTS.md` (quy tắc workflow), `README.md` (cấu trúc + gates), file này, rồi brief của phase sắp làm.

## 1. Đã hoàn thành

| Phase | Phạm vi | Self-review | Kết quả |
|---|---|---|---|
| 01 — Foundation | Tauri 2 + React 19 + Vite + SQLx/SQLite app-data, 2 IPC commands, CSP/capability chặt | `docs/phase-01/self-review.md` | ACCEPT (fix: bỏ serde_json unused, withGlobalTauri:false tường minh) |
| 02 — Design System & UI Shell | 11 tokens Dark Purple, custom titlebar + windowService, AppShell, primitives | `docs/phase-02/self-review.md` | ACCEPT (packaged `tauri build` pass — MSI+NSIS, checksums ghi trong file) |
| 03 — Core UI Screens (prototype) | 9 màn hình + typed mock services + fixture deterministic + test matrix T03-01→13 | `docs/phase-03/self-review.md` (+ `dd5434c`, `13081ee`) | ACCEPT (fix: dialog focus trap, offline filter, task seq vào MockState) |

**Gate hiện tại (toàn bộ xanh):** lint · typecheck · vitest 47/47 · vite build · cargo check/clippy/test (1 warning `Internal` dead_code — chủ đích) · tauri dev smoke exit 0 · tauri build artifacts OK.

## 2. Phase 03 CHƯA phải là app thật

- Toàn bộ dữ liệu là **mock fixture** (`src/services/mock/`), state mất khi reload. Không search/download/CBZ/reader thật, không bảng DB nghiệp vụ (chỉ `app_settings`).
- **Điểm swap duy nhất sang backend:** `src/services/provider.ts` — thay mock bằng IPC-backed service cùng interface, không rework screens. Mapping chi tiết: `docs/phase-03/implementation-notes.md` §8.

## 3. Bất biến kiến trúc (không được phá)

1. Mọi `invoke()` app-command đi qua `src/shared/api/desktop.ts`; window API đi qua `src/services/windowService.ts`.
2. capability chỉ window `main`: `core:default` + 5 window permission tường minh; **không** fs/shell/http; CSP không wildcard — muốn mở quyền phải có brief phase cho phép.
3. SQLite chỉ từ Rust; DB tại `app.path().app_data_dir()` (identifier `com.manga1989.downloader`); migration append-only, không sửa `0001_init.sql` đã release.
4. Lỗi IPC có cấu trúc `{code, message, retryable}` đã sanitize — không path/SQL/stack trace.
5. Health phải thật: `app_health_check` chạy `SELECT 1` thật; UI không hardcode "ready".
6. Không `Math.random()`/clock trong fixture; test phải `resetMockState()` được.
7. `AGENTS.md` là file của user — không sửa/xóa.

## 4. Việc còn tồn (chưa có phase nào giao)

- [ ] Chạy thử **bản cài đặt** (MSI/NSIS trong `src-tauri/target/release/bundle/`, gitignore) + xác nhận titlebar drag/resize/snap, scaling 100–200%, screenshot các màn hình tại 1440×900 và 1024×700 → đính vào docs phase sau.
- [ ] Icon app thật (thay placeholder 16×16): `npx tauri icon <png>` trước khi release.
- [ ] Cân nhắc self-host font Inter (CSP `'self'` cấm CDN).
- [ ] `AppError::Internal` (Rust) sẽ được dùng khi có lỗi nghiệp vụ.

## 5. Bắt đầu Phase 04+ thế nào

1. User cung cấp brief phase mới (các brief hiện có trong `docs/briefs/`).
2. Quy trình đã thống nhất: **inspect repo → implement theo task trong brief → tự review đối chiếu brief (BLOCKING/ADVISORY, sửa ngay BLOCKING) → chạy gates → commit local → report**. Bước reviewer ngoài đã được user bỏ (2026-09-22).
3. Lệnh chuẩn: xem `README.md` §Quality Gates. Không push/merge/deploy khi user chưa yêu cầu rõ.
4. Kết thúc mỗi phase: cập nhật `docs/phase-XX/` + bảng §1 của file này.

## 6. Cấu trúc docs

```
docs/
├── briefs/          # brief gốc Phase 01–03 (nguồn yêu cầu)
├── plans/           # implementation plan đã duyệt Phase 01
├── phase-01|02|03/  # implementation-notes + self-review từng phase
└── HANDOFF.md       # file này
```
