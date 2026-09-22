# Phase 10 — Artifact Manifest (staging RC, actual bytes)

| Trường | Giá trị thật |
|---|---|
| Artifact | `src-tauri/target/release/bundle/nsis/Manga Downloader 1989_1.0.0_x64-setup.exe` |
| Version | 1.0.0 (verified: exe `VersionInfo.ProductVersion/FileVersion = 1.0.0`, `ProductName = Manga Downloader 1989`, `npm run check:version` OK 4/4 files) |
| Architecture | Windows x64 (NSIS, per-user `currentUser`) |
| Size | 4,480,762 bytes |
| SHA-256 (unsigned final bytes) | `6090b532758b3006f8fa0fe5969e00c752cbea6d14e2732e38239efb9d7f97a6` |
| Signing | **UNSIGNED** — không có certificate được ủy quyền (chính sách: `SECURITY_SIGNING_UPDATER.md`) |
| Updater artifacts | Không có (updater tắt có chủ đích; không `.sig`) |
| Source baseline | `30f50be` (Phase 09 + review 3 vòng) + commit Phase 10 này chứa toàn bộ thay đổi/config/docs P10 |
| Build environment | Windows 11 Pro x64 26200 · Node v24.18.0 · rustc 1.98.1 stable-msvc · `npm run tauri build -- --bundles nsis` EXIT=0 |
| App binary smoke | `target/release/manga-downloader-1989.exe` khởi chạy sống 12 s, 0 lỗi log (T10-12/T10-14 một phần — launch từ exe trực tiếp; Start Menu = manual) |
| Installer install-test | **NOT RUN** — cần VM/profile sạch theo `WINDOWS_INSTALL_UPGRADE_TEST.md` (không cài lên máy user đang dùng) |
| Reproduce | `RELEASE_RUNBOOK.md` §Build — cùng commit + lockfiles cho bytes tái lập được; build lại ⇒ hash mới ⇒ re-verify |

**Quy tắc:** mọi phân phối phải đối chiếu SHA-256 này từ kênh chính thức do owner công bố; hash chỉ có hiệu lực với đúng bytes của build nêu trên.
