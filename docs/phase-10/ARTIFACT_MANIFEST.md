# Phase 10 — Artifact Manifest (staging RC, actual bytes)

| Trường | Giá trị thật |
|---|---|
| Artifact | `src-tauri/target/release/bundle/nsis/Manga Downloader 1989_1.0.0_x64-setup.exe` |
| Version | 1.0.0 (verified: exe `VersionInfo.ProductVersion/FileVersion = 1.0.0`, `ProductName = Manga Downloader 1989`, `npm run check:version` OK 4/4 files) |
| Architecture | Windows x64 (NSIS, per-user `currentUser`) |
| Size | 4,490,082 bytes |
| SHA-256 (unsigned final bytes) | `2245f30e625ed5ecbe6c73ad63da4f56182b922d0b31ed5c7def702c22a90674` |
| Signing | **UNSIGNED** — không có certificate được ủy quyền (chính sách: `SECURITY_SIGNING_UPDATER.md`) |
| Updater artifacts | Không có (updater tắt có chủ đích; không `.sig`) |
| Source baseline | Tree của commit review toàn project 4 vòng (trên nền `30f50be` + Phase 10 + `d272dd6`/`90e9654`): gồm CSP bỏ `asset:`, shutdown không cancel, pause reject verifying/exporting, repair migration 0004, cancel-tới-trước-commit, has_missing semantics, queue format reuse, library temp sweep, frontend monotonic-merge + blob-leak + DEV-gate Sources. Commit chứa đúng tree này được ghi ở `docs/HANDOFF.md` §1a. |
| History | Build RC đầu tiên (4,480,762 B / `6090b532…f97a6`, baseline `30f50be`+P10) **SUPERSEDED** — không phân phối hash cũ; evidence cũ `evidence/rc-verification-2026-09-23.txt` là snapshot pre-rebuild |
| Build environment | Windows 11 Pro x64 26200 · Node v24.18.0 · rustc 1.98.1 stable-msvc · `npm run tauri build -- --bundles nsis` EXIT=0 |
| App binary smoke | `target/release/manga-downloader-1989.exe` khởi chạy sống 12 s, 0 lỗi log (T10-12/T10-14 một phần — launch từ exe trực tiếp; Start Menu = manual) |
| Installer install-test | **NOT RUN** — cần VM/profile sạch theo `WINDOWS_INSTALL_UPGRADE_TEST.md` (không cài lên máy user đang dùng) |
| Reproduce | `RELEASE_RUNBOOK.md` §Build — cùng commit + lockfiles cho bytes tái lập được; build lại ⇒ hash mới ⇒ re-verify |

**Quy tắc:** mọi phân phối phải đối chiếu SHA-256 này từ kênh chính thức do owner công bố; hash chỉ có hiệu lực với đúng bytes của build nêu trên.
