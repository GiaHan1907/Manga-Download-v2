# Phase 01 Self-Review (theo brief PHASE_01_IMPLEMENTATION_PLAN.md)

**Ngày review:** 2026-09-22 · **Hình thức:** tự review đối chiếu brief (workflow mới, thay bước reviewer ngoài)
**Phạm vi review:** các hạng mục Phase 01 trên trạng thái code hiện tại (sau Phase 02–03).

## 1. Bằng chứng đã kiểm tra

| Yêu cầu brief | Kiểm chứng | Kết quả |
|---|---|---|
| §3.2/§6 Mọi `invoke()` qua `desktop.ts` | grep `invoke|@tauri-apps` trong `src/` | ✅ chỉ `shared/api/desktop.ts` (app IPC); `windowService.ts` dùng window API — do brief Phase 02 tách riêng, không phải app command |
| §3.3/§7 SQLite app-data, pool một lần, migration nhúng | `db/mod.rs`: `create_dir_all`, `create_if_missing`, `foreign_keys(true)`, `sqlx::migrate!`, max 5 connections, **không WAL** | ✅ |
| §3.7/§6 Lỗi IPC có cấu trúc, đã sanitize | `error.rs`: wire `{code, message, retryable}`, message cố định tiếng Việt, không path/SQL/stack | ✅ |
| §6 Health chỉ báo ready khi pool sống | `app_health_check` chạy `SELECT 1` thật trước khi trả `ready` | ✅ |
| §7 Migration đúng DDL, không seed | `0001_init.sql` chỉ tạo `app_settings` | ✅ |
| §8 T6 Capability/CSP chặt | `capabilities/default.json` chỉ window `main`; CSP không wildcard, không remote script | ✅ (5 permission window bổ sung ở Phase 02 là compact được brief Phase 02 cho phép) |
| §12 Không ghi đè file user | `git diff 505781c..HEAD -- AGENTS.md` = **rỗng** | ✅ |
| §12 Migration idempotent, test không chạm DB user | `cargo test` 2/2 (tempdir); app src-tauri không đổi từ Phase 01 | ✅ |
| §5 Lockfiles commit, artifacts ignore | `.gitignore` loại db/dist/target, giữ `package-lock.json`/`Cargo.lock` | ✅ |
| §9 Scripts đủ | dev/build/preview/lint/typecheck/test/test:watch/tauri | ✅ |

## 2. Findings

| # | Finding | Phân loại | Xử lý |
|---|---|---|---|
| F1 | `serde_json` khai báo trong `Cargo.toml` nhưng **không dùng** ở bất kỳ file Rust nào (grep = 0) — vi phạm chính check §12 "có dependency không dùng không" | BLOCKING | ✅ Đã bỏ khỏi dependencies; `cargo check/clippy/test` pass, `Cargo.lock` cập nhật |
| F2 | `withGlobalTauri` không khai báo tường minh (mặc định đã là `false`, nhưng §8 T6 nên verify được bằng config) | BLOCKING (nhẹ) | ✅ Thêm `"withGlobalTauri": false` tường minh; schema validate qua `cargo check` |
| F3 | `tauri::async_runtime::block_on` trong setup hook — check §12 "DB init có block UI không?" | ADVISORY | Giữ: setup chạy **trước khi** webview tồn tại; công việc SQL thật chạy trên tokio worker; `block_on` chỉ chờ tạo pool + migration (chục ms). Không có UI thread nào bị chặn |
| F4 | `AppError::Internal` chưa được dùng (dead_code warning) | ADVISORY | Giữ nguyên như handoff gốc; sẽ dùng khi có lỗi nghiệp vụ Phase 04 |
| F5 | `style-src 'unsafe-inline'` trong CSP | ADVISORY | Chấp nhận có chủ đích: React dùng inline style; `script-src 'self'` vẫn chặt, không có `*` |
| F6 | Window size/decorations hiện tại (1440×900, min 1024×700, `decorations:false`) khác cấu hình gốc Phase 01 | ADVISORY | Thay đổi hợp lệ theo brief Phase 02 §3 đã duyệt; không phải hồi quy |

## 3. Definition of Done §11 — đánh giá lại

- [x] Repo chạy theo README, package manager/lockfile nhất quán (npm + `package-lock.json`)
- [x] Tauri 2 window khởi động; React+TS+Vite
- [x] Shell Dark Purple hiển thị app info + trạng thái hệ thống (nay là panel trong Home, vẫn đúng tinh thần)
- [x] IPC wrapper duy nhất, 2 command đăng ký tường minh
- [x] SQLite app-data + migration `0001_init.sql` (đã xác nhận runtime: DB tạo tại `%APPDATA%\com.manga1989.downloader\`)
- [x] Capability/CSP không quyền fs/network/shell ngoài phạm vi được duyệt
- [x] Lint/typecheck/test/build + cargo check/clippy/test pass (kết quả run gần nhất, 2026-09-22)
- [x] README + handoff mô tả trung thực (bao gồm `tauri build` not run)
- [x] Không có tính năng ngoài Phase 1 được tuyên bố đã làm

**Kết luận self-review Phase 01: ACCEPT** sau khi sửa F1, F2. Không còn BLOCKING mở.
