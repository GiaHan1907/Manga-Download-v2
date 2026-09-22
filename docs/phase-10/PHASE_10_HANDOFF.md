# Phase 10 Handoff — Manga Downloader 1989

## 1. Status

**`RC_TESTED` (build-level) — RELEASE CANDIDATE READY · PUBLICATION BLOCKED.**
Chưa `CODEX_REVIEW_ACCEPTED`, chưa `AUTHORIZED_RELEASE`. Không có tuyên bố production release nào trong file này.

## 2. Phase 09 ACCEPT reference & reviewed source

- Phase 09: `docs/phase-09/PHASE_09_HANDOFF.md` (+ §14 review 3 vòng; matrix 45 PASS/PASS(code), 5 PARTIAL, 1 NOT RUN, 1 BLOCKED, 0 FAIL).
- ACCEPT model: quy trình user-driven "review N vòng" (reviewer ngoài đã bỏ 2026-09-22, `docs/HANDOFF.md` §5). ACCEPT cuối cho RC Phase 10 = user/Codex Reviewer (T10-39 BLOCKED chờ quyết định này).
- Baseline: `30f50be`; toàn bộ thay đổi P10 trong commit Phase 10 này.

## 3. P10-01..10 status & deviations

| Task | Status | Ghi chú |
|---|---|---|
| P10-01 audit | xong | `RELEASE_CONFIGURATION.md` — decision record khóa trước code |
| P10-02 identity/version | xong | 1.0.0 đồng bộ 4 nguồn + `check-version.mjs` (T10-05 chứng minh bắt drift); icon set mới từ `scripts/make-icon.mjs` (thay placeholder 16×16 — pending item cũ) |
| P10-03 NSIS | xong ở mức build | targets đổi `["msi","nsis"]` → `["nsis"]` (decision §3 plan); publisher/install currentUser; staging build EXIT=0; install-test thật = NOT RUN theo rule (VM sạch, checklist có sẵn) |
| P10-04 signing | BLOCKED-by-policy | không cert → unsigned nhất quán mọi tài liệu |
| P10-05 updater | branch B | tắt; manual upgrade = cài đè; không key/endpoint giả |
| P10-06 CI | xong(config) | `.github/workflows/ci.yml` + `release-windows.yml` (environment `release-staging`, không publish); chưa có run thật trên GitHub (repo chưa push) |
| P10-07 install/upgrade/uninstall | NOT RUN (checklist) | `WINDOWS_INSTALL_UPGRADE_TEST.md` 15 mục cho user/VM |
| P10-08 manifest/docs/runbook | xong | `ARTIFACT_MANIFEST.md`, `RELEASE_RUNBOOK.md`, `RELEASE_NOTES.md`, `KNOWN_ISSUES.md`, `SECURITY_SIGNING_UPDATER.md` |
| P10-09 RC verification | xong phần automated | gates cuối trên candidate (mục 5); matrix T10-01..40 (`RELEASE_TEST_MATRIX.md`) |
| P10-10 publish | **BLOCKED** | chờ owner approval + destination + Release Engineer; agent không push/tag/publish |

**Deviations so với plan:** không tạo MSI (chủ đích, plan cho phép NSIS-only); không Playwright/E2E mới (tái dùng suite đã ACCEPT); icon sinh bằng script nội bộ vì ImageGen API hết quota — provenance nằm trong repo.

## 4. Environment

Windows 11 Pro x64 Build 26200 · Node v24.18.0 · rustc 1.98.1 stable-msvc · @tauri-apps/cli 2.x · NSIS (bundle qua tauri CLI).

## 5. Commands & exit codes (final candidate run, 2026-09-23)

`npm run typecheck` 0 · `npm run lint` 0 · `npx vitest run` **81/81** · `npm run build` 0 · `cargo fmt --check` 0 · `cargo clippy --all-targets -- -D warnings` 0 · `cargo test` **123/123** · `npm run check:version` OK 1.0.0 (+ drift-capture exit 1) · `npm run tauri build -- --bundles nsis` **EXIT=0** · release exe smoke 12 s sống ổn.

## 6. Release identity

commit baseline `30f50be` + P10 commit · version **1.0.0** · tag đề xuất khi được duyệt: `v1.0.0` (chưa tạo) · product `Manga Downloader 1989` · identifier `com.manga1989.downloader` (không đổi — giữ đường dẫn dữ liệu).

## 7. Artifact manifest

`Manga Downloader 1989_1.0.0_x64-setup.exe` · 4,480,762 B · SHA-256 `6090b532758b3006f8fa0fe5969e00c752cbea6d14e2732e38239efb9d7f97a6` · **UNSIGNED** · chi tiết: `ARTIFACT_MANIFEST.md`.

## 8. WebView2 policy

`downloadBootstrapper` mặc định (tài liệu hóa); offline-first chưa phải yêu cầu → offlineInstaller ghi là option tương lai. Hành vi máy thiếu runtime = mục 5 checklist, NOT RUN.

## 9. Upgrade / data preservation

Không có published version trước → T10-22 **N/A** (không fake N-1). Migration chain 0001→0005 forward-only đã test (P09). Cài đè cùng identifier giữ app-data; uninstall không xóa dữ liệu (thiết kế NSIS template mặc định) — xác nhận thực tế thuộc checklist mục 10.

## 10. Updater

TẮT (branch B). Không plugin, không key, không endpoint. Manual upgrade = installer mới cài đè (runbook). Điều kiện bật lại: endpoint + key custody + test N→N+1 VM — tài liệu hóa trong `SECURITY_SIGNING_UPDATER.md`.

## 11. CI & permissions

Hai workflow, cả hai `permissions: contents: read`; PR job không secret/không artifact publish; release job chạy trong environment `release-staging` (chủ repo phải tạo + cấu hình required reviewers trước khi dùng) và upload **staging artifact** duy nhất — không có bước publish nào tồn tại trong file.

## 12. Test matrix

`RELEASE_TEST_MATRIX.md`: PASS 20 · PARTIAL 3 · NOT RUN 8 · N/A 3 · BLOCKED 4 · FAIL 0.

## 13. Security / license / known issues

Không Critical/High mở; K-1..K-12 trong `KNOWN_ISSUES.md`; license audit chi tiết cho phân phối công khai là việc của publish gate (ghi chú runbook), hiện mới xác minh stack attribution.

## 14. Codex Reviewer verdict

(để Reviewer điền: ACCEPT/REJECT + hash đã review `6090b532…f97a6` + commit được chỉ định.)

## 15. Owner release approval & Release Engineer action

**CHƯA THỰC HIỆN** — không tồn tại approval/destination trong phiên này.

## 16. Remote release location & hash verification

**KHÔNG CÓ** — chưa publish; mọi tuyên bố ngược lại sẽ là sai.

## 17. Remaining blockers & maintenance

1. **T10-13..17/20-26 (manual install tests)** — cần VM/profile sạch, làm theo `WINDOWS_INSTALL_UPGRADE_TEST.md`.
2. **T10-18 live source journey** (= R-1 từ P09) — user chạy 1 chương thật, ghi kết quả vào evidence.
3. **T10-39/40** — Reviewer ACCEPT + owner approval + Release Engineer publish + verify remote hash.
4. Nếu phân phối công khai: quyết định cert (K-1), license audit, cân nhắc updater plan riêng.
5. Maintenance: mỗi release chạy `check:version`, rebuild + re-hash + cập nhật manifest; checklist VM tối thiểu mục 1-4, 9-11.
