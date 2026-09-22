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

> **Reconcile (review toàn project 4 vòng, cùng ngày):** các con số trên là snapshot của candidate `6090b532…` TRƯỚC vòng review. Sau review, tree đổi (engine/worker/migration/frontend — xem `docs/HANDOFF.md` §1a) và RC được build lại: gates hiện hành **vitest 85/85 · cargo 134/134**, artifact mới `2245f30e…a90674` (4,490,082 B). Bảng §7 dưới đây ghi giá trị hiện hành.

## 6. Release identity

commit baseline `30f50be` + P10 commit · version **1.0.0** · tag đề xuất khi được duyệt: `v1.0.0` (chưa tạo) · product `Manga Downloader 1989` · identifier `com.manga1989.downloader` (không đổi — giữ đường dẫn dữ liệu).

## 7. Artifact manifest

`Manga Downloader 1.0.0` installer — giá trị HIỆN HÀNH sau rebuild 4 vòng: 4,490,082 B · SHA-256 `2245f30e625ed5ecbe6c73ad63da4f56182b922d0b31ed5c7def702c22a90674` · **UNSIGNED** · chi tiết + history superseded hash `6090b532…`: `ARTIFACT_MANIFEST.md`.

## 8. WebView2 policy

`downloadBootstrapper` mặc định (tài liệu hóa); offline-first chưa phải yêu cầu → offlineInstaller ghi là option tương lai. Hành vi máy thiếu runtime = mục 5 checklist, NOT RUN.

## 9. Upgrade / data preservation

Không có published version trước → T10-22 **N/A** (không fake N-1). Migration chain 0001→0005 forward-only đã test (P09). Cài đè cùng identifier giữ app-data; uninstall không xóa dữ liệu (thiết kế NSIS template mặc định) — xác nhận thực tế thuộc checklist mục 10.

## 10. Updater

TẮT (branch B). Không plugin, không key, không endpoint. Manual upgrade = installer mới cài đè (runbook). Điều kiện bật lại: endpoint + key custody + test N→N+1 VM — tài liệu hóa trong `SECURITY_SIGNING_UPDATER.md`.

## 11. CI & permissions

Hai workflow, cả hai `permissions: contents: read`; PR job không secret/không artifact publish; release job chạy trong environment `release-staging` (chủ repo phải tạo + cấu hình required reviewers trước khi dùng) và upload **staging artifact** duy nhất — không có bước publish nào tồn tại trong file.

## 12. Test matrix

`RELEASE_TEST_MATRIX.md`: **PASS 25** (8 PASS có điều kiện config/fixture/doc, runtime tương ứng NOT RUN được ghi ngay trong dòng đó) · **PARTIAL 2** · **NOT RUN 6** · **N/A 3** · **BLOCKED 4** · **FAIL 0** (tổng 40, đếm từ các dòng của ma trận — bản tóm tắt cũ 20/3/8/3/4 sai số học, đã sửa ở review vòng 2).

## 13. Security / license / known issues

Không Critical/High **defect** mở. Phân biệt rõ: **K-3 (HIGH) là verification gap** — installer/live-journey chưa ai chạy trên máy sạch — và chính nó là lý do T10-13..18/24..26 vẫn NOT RUN/BLOCKED; nó chặn ACCEPT/publish, không phải bug đã biết bị che. K-1..K-12 trong `KNOWN_ISSUES.md`; license audit chi tiết cho phân phối công khai là việc của publish gate (ghi chú runbook), hiện mới xác minh stack attribution.

## 14. Codex Reviewer verdict

(để Reviewer điền: ACCEPT/REJECT + hash đã review `2245f30e…a90674` (hash cũ `6090b532…f97a6` đã superseded bởi rebuild 4 vòng) + commit được chỉ định.)

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

## 18. Review 3 vòng (sau commit RC `d272dd6`)

- **Vòng 1 (config/scripts/CI):** `check-version.mjs` chuyển từ `URL.pathname` (vỡ với path có khoảng trắng/Unicode do percent-encoding) sang `fileURLToPath`; manifest release workflow sửa để lấy version từ `tauri.conf.json` (dispatch không còn ghi nhầm tag="master"); chú thích re-pin actions theo plan §8 vào cả 2 workflow; icon determinism chứng minh (regen 2 lần cùng SHA-256). Gates xanh.
- **Vòng 2 (docs-vs-chính-nó):** **BLOCKING — summary ma trận T10 sai số học lần 2** (ghi 20/3/8/3/4 = 38 ≠ 40; đếm thật: 25 PASS trong đó 8 có điều kiện · 2 PARTIAL · 6 NOT RUN · 3 N/A · 4 BLOCKED · 0 FAIL) — sửa 3 tài liệu; đối thuẫn K-3 "High" vs câu "không Critical/High mở" được diễn đạt lại (K-3 là verification gap chặn ACCEPT, không phải defect bị che); typo runbook.
- **Vòng 3 (adversarial):** re-hash installer trên đĩa = khớp manifest từng byte (4,480,762 B / `6090b532…f97a6`); drift test chạy lại với exit code trung thực (1 khi lệch, 0 khi khôi phục — capture lỗi tiên nghiệm bằng pipe đã bị che exit code, sửa trong evidence); quét secrets: workflows 0 tham chiếu secret, repo 0 file key/cert. Toàn bộ bằng chứng: `evidence/rc-verification-2026-09-23.txt`.
- **Kết luận:** 0 BLOCKING code/runtime tồn tại; mọi finding thuộc loại portability-docs-honesty và đã sửa. Gates chốt sau 3 vòng: vitest 81/81 · cargo 123/123 · fmt/clippy · typecheck/lint/build · `check:version` OK. Trạng thái giữ nguyên: **RC_TESTED — PUBLICATION BLOCKED**.
- **Bổ sung — review TOÀN PROJECT 4 vòng (ngay sau đó, user yêu cầu):** vòng 1 security/bất biến (CSP bỏ `asset:` dead-allowance; validate_relpath trước khi probe path từ DB), vòng 2 engine/data (shutdown không cancel; cancel tới trước commit; verify_cbz hết panic offset; repair migration 0004 giữ checksum; has_missing đúng in-app delete; BEGIN IMMEDIATE; sweep temp trong library; queue reuse nhận format), vòng 3 frontend (action table theo lifecycle mới; monotonic merge chống stale-snapshot; blob leak post-unmount; Sources sandbox gate DEV), vòng 4 docs↔code + rebuild RC. Gates hiện hành: **vitest 85/85 · cargo 134/134**, artifact `2245f30e…a90674`. Chi tiết: `docs/HANDOFF.md` §1a. Trạng thái release KHÔNG đổi: RC_TESTED — PUBLICATION BLOCKED.
