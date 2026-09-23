# Phase 09 Handoff — Manga Downloader 1989

## 1. Summary & commit baseline

Baseline: `71c5ba8` (Phase 08 + review 3 vòng, working tree này = Phase 09 candidate). Phase 09 = Testing/Security/Performance: **không đổi stack, không migration mới, không thêm capability**, chỉ +17 test Rust (tổng 123), 1 bug-fix testkit (u8 overflow), 2 log-sanitize, và toàn bộ hồ sơ evidence `docs/phase-09/`.

## 2. Phase 08 ACCEPT evidence & deviations

- Phase 08: `docs/phase-08/self-review.md` (3 vòng, 9 BLOCKING đã sửa) + commit `71c5ba8`; chờ xác nhận CROSS của Codex cho chuỗi P07→P09 theo quy trình AGENTS.md.
- Sai lệch so với plan P09: **không có live-source test** (môi trường dev chặn mạng → tất cả E2E dùng fixture/mocked, đúng rule §2.5); **không có Playwright** (repo không cấu hình; thay bằng vitest+jsdom + manual checklist). Không có gì được âm thầm đổi.

## 3. Tasks P09-01..09 status

| Task | Status | Đầu ra chính |
|---|---|---|
| P09-01 Audit/baseline | xong | gates ghi nguyên văn; inventory 2 tác tử Explore |
| P09-02 Deterministic infra | xong (tái dùng) | `perf.rs`, `RawZip` builder, seed 1k×10k, `test_pool()` convention |
| P09-03 E2E regression | xong ở mức automated hiện có | map T09-04..15 (QA_MATRIX) |
| P09-04 Reliability | xong; 20-task đồng thời để mở (R-4) | crash boundaries A/B, dedup, retry budget đều có test |
| P09-05 DB/storage integrity | xong | +concurrent writers, +Unicode slug, corrupt settings blob |
| P09-06 Security | xong | `SECURITY_REVIEW.md` S-1..S-6; 10 test adversarial mới |
| P09-07 Performance | xong | `PERFORMANCE_BASELINE.md`: 9.7 ms/4.9 ms/18.9 ms/0.12 ms; checkpoint fsync ghi nhận R-3 |
| P09-08 Windows | một nữa | `tauri build` EXIT=0 + release exe smoke PASS; manual checklist NOT RUN (`WINDOWS_SMOKE.md`) |
| P09-09 Stabilization/handoff | file này | full regression T09-52 PASS |

## 4. Files modified / new fixtures / new tests

- `src-tauri/src/perf.rs` (MỚI, cfg(test)): 3 benchmark tripwire.
- `reader/cbz.rs`: +7 adversarial unit tests (RawZip builder) — file từng 0 test.
- `sources/http.rs`: +3 tests (classify/Retry-After, DNS guard fail-closed, pacing đo thật ≥2.6s, mock cap).
- `services/settings_service.rs`: +1 corrupt-blob fallback test. `db/mod.rs`: +1 concurrent-writers test. `downloader/storage.rs`: +1 Unicode slug test.
- `reader/testkit.rs`: sửa u8-overflow (bug thật tìm thấy nhờ perf fixture 300 trang).
- Log sanitize: `library_service.rs`, `storage_service.rs`, `lib.rs` (io error → kind()).
- Docs: 6 hồ sơ phase-09 + evidence (checksums, audit).

## 5. QA Matrix summary

**PASS/PASS(code): 45** · **PARTIAL: 5** (T09-18 20-task, T09-23 disk-full OS-level, T09-47 RAM dài hơi, T09-48 20-task end-to-end, T09-51 fullscreen/reduced-motion) · **NOT RUN: 1** (T09-49 display matrix) · **BLOCKED-by-design: 1** (T09-32 đổi root). Không FAIL. Chi tiết: `QA_MATRIX.md`.

## 6. Commands / OS / toolchain / evidence

Windows 11 Pro x64 26200 · Node 24.18.0 · rustc 1.98.1 · lệnh §6 plan chạy nguyên scripts repo (`npm run lint|typecheck|test|build`, `cargo fmt/clippy -D warnings/test`, `npm run tauri build`); evidence: `docs/phase-09/evidence/{audit-2026-09-23.txt,bundle-checksums-2026-09-23.txt}`.

## 7. Security findings & mitigations

S-1..S-6 trong `SECURITY_REVIEW.md` — 4 đã sửa (test-gap 3 + logging 1), 2 chấp nhận có lý do (S-5 GuardedHttp E2E cap, S-6 rsa advisory không nằm trong Windows tree). Không Critical/High mở. Không mở rộng capabilities/CSP.

## 8. Performance baseline & hardware

`PERFORMANCE_BASELINE.md` — ngưỡng plan đều đạt cách xa; tripwires khoá trong suite; fsync-per-checkpoint (4 ms/write) được ghi là đánh đổi có chủ đích (R-3).

## 9. DB migration / storage integrity evidence

Không migration mới; fresh + upgrade-replay tests (`t06_01`, `t04_02`), FK/unique/txn/busy (`t09_concurrent_writers...`), reconcile/cleanup/delete không phá huỷ (`t07_*`, `t08_clear_cache...`), Unicode path (`t09_unicode...`).

## 10. Windows smoke evidence

Tự động: dev smoke nhiều lần, `tauri build` EXIT=0 (bundle lần đầu kể từ Phase 04), release exe chạy 12 s. Thủ công: checklist 13 mục — **toàn bộ NOT RUN**, chờ user (`WINDOWS_SMOKE.md`).

## 11. Known issues / release blockers

R-1 live journey: ĐÓNG 2026-09-23 qua headless probe trên máy user (Ninja ch.1, 188/188 trang CBZ 47,3 MiB verify — xem RISK_REGISTER R-1); còn xác nhận lại trong app UI; R-2 installer chưa cài-thử; R-3..R-8 chấp nhận có ghi chú; R-6 reduced-motion + deflate CBZ + TanStack-kiến-trúc (đã deviate có lý do P08) = debt có kiểm soát.

## 12. Phase 10 handoff requirements

- Lockfile + toolchain pin như mục 6; build instructions: `npm ci && npm run tauri build` (NSIS khuyến nghị — file 4.5 MB; MSI cần WiX tại build time).
- Chạy manual checklist WINDOWS_SMOKE trước khi ký/phát hành; điền PASS/FAIL + screenshot vào `evidence/`.
- Release notes nên nêu: CBZ STORE-only, library root cố định trong app-data, notifications cần Windows toast permission (từ chối = app vẫn chạy im lặng).
- Migration tương thích: DB schema 5 — backup file `manga-downloader-1989.db` là đủ (append-only).
- Signing/updater: CHƯA làm gì (đúng ranh giới P09/P10).

## 13. Codex Reviewer decision

(để Reviewer điền: ACCEPT/REJECT + findings phân loại.)

## 14. Review 3 vòng (làm sau handoff ban đầu, commit này gồm luôn)

- **Vòng 1 (test correctness):** đối chiếu `RawZip` builder với từng offset thực của parser (central method@10/size@20/namelen@28/offset@42, EOCD count@10/len@12/offs@16) — baseline test `t09_handcrafted...` chứng minh builder đúng nên mọi rejection là của guard, không phải artifact; http tests không tautological (hàm thật, resolver thật, đo thời gian thật). **Không BLOCKING.**
- **Vòng 2 (docs vs code):** toàn bộ 17 tên test được trích dẫn trong docs tồn tại trong code (verify grep); nhưng **số học ma trận sai ở 3 docs** (ghi 40 PASS/4 PARTIAL; thực đếm được 45 PASS/PASS(code) + 5 PARTIAL + 1 NOT RUN + 1 BLOCKED) → đã sửa QA_MATRIX, PHASE_09_HANDOFF §5, docs/HANDOFF.md. **1 BLOCKING (doc-truthfulness).**
- **Vòng 3 (adversarial/evidence):** mục "perf numbers" trong `evidence/gate-outputs` rỗng (grep sai pattern) → regenerate file với output `--nocapture` thật + ghi chú variance re-run (<25%) vào PERFORMANCE_BASELINE. **1 BLOCKING (evidence completeness).**
- Gates sau mỗi vòng; chốt: vitest 81/81 · cargo 123/123 · fmt/clippy sạch (`evidence/gate-outputs-2026-09-23.txt`).
