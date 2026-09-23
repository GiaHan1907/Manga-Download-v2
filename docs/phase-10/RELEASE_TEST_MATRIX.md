# Phase 10 — Release Test Matrix (T10-01..T10-40)

**Candidate:** source `30f50be` + commit Phase 10 này · version 1.0.0 · artifact `Manga Downloader 1989_1.0.0_x64-setup.exe` SHA-256 `6090b532…f97a6` (4,480,762 B, unsigned). **RECONCILE:** toàn bộ bảng này mô tả candidate gốc; sau review toàn project 4 vòng, tree đổi và RC rebuild → artifact hiện hành `2245f30e…a90674` (4,490,082 B), gates hiện hành vitest 85/85 · cargo 134/134 (xem `ARTIFACT_MANIFEST.md` + `docs/HANDOFF.md` §1a). Các dòng test tự động bên dưới đã chạy LẠI trên tree mới và vẫn PASS với số lớn hơn.
**Quy tắc:** PASS = đã chạy thật có bằng chứng; BLOCKED = cần hạ tầng/quyền/chủ thể khác; NOT RUN = chưa ai thao tác; N/A = capability không tồn tại trong cấu hình release đã khóa. Không mục nào được tô hồng.

| ID | Test | Trạng thái | Bằng chứng / ghi chú |
|---|---|---|---|
| T10-01 | Phase 09 ACCEPT thật | PASS(within-process) | `docs/phase-09/PHASE_09_HANDOFF.md` + §14 review 3 vòng; gates P09 xanh; ACCEPT nội bộ theo quy trình user (reviewer ngoài đã bỏ 2026-09-22) — **chốt cuối vẫn chờ user/Codex cho chính RC này** |
| T10-02 | Working tree hiểu đúng, không discard thay đổi user | PASS | `git status` sạch trước khi sửa; baseline `30f50be`; mọi thay đổi P10 nằm trong 1 commit |
| T10-03 | Display name + bundle identifier khớp config đã duyệt | PASS | `productName`/`identifier` không đổi (`RELEASE_CONFIGURATION.md`); exe `ProductName = Manga Downloader 1989` |
| T10-04 | Version invariant | PASS | `npm run check:version` → OK 1.0.0 (4/4 nguồn) |
| T10-05 | Script phát hiện mismatch chủ đích | PASS | Chạy thật: tạm lệch version trong package.json → exit 1 "VERSION DRIFT"; khôi phục (evidence trong log phiên P10) |
| T10-06 | Icon/taskbar/Start Menu/About reflect identity | PARTIAL | Icon set mới sinh + `tauri icon` (source trong repo); exe metadata đúng; About panel hiện version qua IPC — **Start Menu/taskbar thật = NOT RUN** (mục 3 checklist) |
| T10-07 | Frozen install từ lockfile | PASS | `npm ci` chạy trong build P10 (staging build dùng cùng workspace sạch); lockfile không đổi kể từ P09 |
| T10-08 | Frontend lint + typecheck | PASS | cổng cuối trên candidate (xem `PHASE_10_HANDOFF.md` §5) |
| T10-09 | Frontend tests | PASS | vitest 81/81, không skip mới |
| T10-10 | Rust fmt/clippy/test target release | PASS | fmt OK, clippy `-D warnings` 0, cargo test 123/123 |
| T10-11 | Production frontend build | PASS | vite build OK (856 ms) |
| T10-12 | Native Windows x64 Tauri release build | PASS | `npm run tauri build -- --bundles nsis` EXIT=0 — NSIS-only theo decision đã khóa |
| T10-13 | NSIS cài trên profile Windows sạch | **NOT RUN** | Agent không cài lên máy user (rule §2.4); checklist `WINDOWS_INSTALL_UPGRADE_TEST.md` |
| T10-14 | App launch từ shortcut Start Menu/Desktop | PARTIAL | exe trực tiếp chạy sống 12 s (bằng chứng T10-12 run); shortcut = NOT RUN (thuộc mục 3) |
| T10-15 | WebView2 có sẵn: cài/launch | NOT RUN | Kịch bản A checklist (Win11 máy dev có runtime — nhưng cài-by-installer chưa chạy) |
| T10-16 | WebView2 thiếu: bootstrap hành vi | NOT RUN | Kịch bản B checklist; policy `downloadBootstrapper` đã document |
| T10-17 | Per-user install không cần admin | PASS(config)/NOT RUN(runtime) | `installMode: currentUser` trong conf; hành vi thực tế thuộc mục 1 |
| T10-18 | Release app dùng nguồn test (network) | PARTIAL | R-1 đóng ở mức headless 2026-09-23 (probe live, nguồn thật truyentuoitho — 188 trang verified); re-verify trên build release + thao tác UI trong app còn mở |
| T10-19 | Tải fixture chapters → CBZ valid (release build) | PASS(fixture-level) | engine/exporter tests 123/123 chạy trên code release; live = T10-18 |
| T10-20 | Library/Reader offline, không mạng | PASS | reader tests không network + P09 offline evidence; installer-level offline = mục 6 |
| T10-21 | Cài đặt không reset SQLite/Favorites/Settings/progress | PASS(config)/NOT RUN(installer) | uninstall/install không đụng app-data theo design Tauri NSIS (không data-removal step trong template); xác nhận thực tế = mục 9/10 |
| T10-22 | Upgrade từ phiên bản published trước | **N/A** | Không tồn tại release nào (`git tag` rỗng) — không tạo fake N-1 (plan §5.5); schema migration đã test riêng (P09 T09-27) |
| T10-23 | Queue gián đoạn sau upgrade an toàn/trung thực | PASS(recovery-level) | recovery tests (t06_15/16, crash boundaries); installer-upgrade thực = mục 8 |
| T10-24 | Uninstall giữ dữ liệu user | NOT RUN | mục 10 checklist |
| T10-25 | Reinstall nhận dữ liệu cũ, không destructive | NOT RUN | mục 11 checklist |
| T10-26 | Installer hỏng/đĩa thấp fail gracefully | NOT RUN | mục 12/13 checklist |
| T10-27 | Capabilities/CSP production vẫn least-privilege | PASS | `capabilities/default.json` + CSP không đổi (diff P10 chỉ version/bundle/icon) |
| T10-28 | Không secrets/đường dẫn cá nhân/key trong bundle-log-manifest | PASS | quét `*.pem/*.key/*_rsa*` = 0; workflow grep `secrets.` = 0; docs không chứa đường dẫn user (chỉ `%APPDATA%\com.manga1989...`) |
| T10-29 | Authenticode verify (nếu bật) | **BLOCKED** | Không có certificate/ signer được ủy quyền — chính sách `SECURITY_SIGNING_UPDATER.md` |
| T10-30 | Unsigned được dán nhãn trung thực | PASS | manifest + release notes + matrix đều ghi UNSIGNED, không hứa SmartScreen |
| T10-31 | Updater N→N+1 (nếu bật) | **N/A** | Updater tắt có chủ đích (branch B) — không key/endpoint giả |
| T10-32 | Updater reject signature sai (nếu bật) | **N/A** | như trên |
| T10-33 | Updater tắt → không prompt cụt; manual upgrade được test/document | PASS(doc)/NOT RUN(test) | Runbook §Upgrade định nghĩa đường cài đè; test thật cần 2 installer bản khác nhau — sẽ chạy ở release kế tiếp (N-1 đầu tiên tồn tại từ 1.0.0 này) |
| T10-34 | PR CI không có publish/signing secret | PASS(config) | `ci.yml` permissions `contents: read`, không env/secret tham chiếu; chưa chạy trên GitHub thật (repo chưa push) → hành vi = NOT RUN |
| T10-35 | Gate fail chặn release | PASS(local)/NOT RUN(CI) | Chuỗi `&&` local fail-loud; workflow kế thừa cùng gates trước build; GitHub-side enforcement chưa có bằng chứng run |
| T10-36 | SHA-256 khớp FINAL published bytes | PASS | hash tính trên file NSIS cuối cùng (unsigned = final hiện tại); quy trình re-hash sau signing đã ghi |
| T10-37 | Release notes/requirements/licenses chính xác | PASS | `RELEASE_NOTES.md` (ràng buộc khai thẳng); attribution stack trong notes; license audit chi tiết dependency = việc publish công khai (ghi chú runbook) |
| T10-38 | Backup/recovery không auto-xóa library, không hứa downgrade DB | PASS | Runbook §Backup — copy khi app đóng, restore forward-migrate, downgrade được khai KHÔNG bảo đảm |
| T10-39 | Codex Reviewer ACCEPT đúng commit/manifest | **BLOCKED** | chờ reviewer/user — trạng thái hiện: RC chờ review |
| T10-40 | Publish sau owner approval, verify remote hash | **BLOCKED** | Chưa có destination được duyệt + chưa có quyền; agent không push/tag/publish |

**Tổng (đếm từ chính 40 dòng trên):** PASS 25 — trong đó 8 PASS có điều kiện ghi rõ mức config/fixture/doc mà runtime tương ứng còn NOT RUN (T10-01, 17, 19, 21, 23, 33, 34, 35) · PARTIAL 2 (T10-06, T10-14) · NOT RUN 6 (T10-13, 15, 16, 24, 25, 26) · N/A 3 (T10-22, 31, 32) · BLOCKED 4 (T10-18, 29, 39, 40) · **FAIL 0**.
**Kết luận state machine:** `RC_TESTED` (build-level) — chưa `CODEX_REVIEW_ACCEPTED`, chưa `AUTHORIZED_RELEASE`. Xuất bản công khai bị chặn hợp lệ bởi T10-39/T10-40.
