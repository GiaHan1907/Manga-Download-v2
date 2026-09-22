# Phase 10 — Release Configuration (decision record, P10-01/P10-02)

**Ngày:** 2026-09-23 · **Baseline source:** `30f50be` (Phase 09 + review 3 vòng) → candidate commit Phase 10 này.

## Tiền đề gate

- Phase 09: toàn bộ gates tự động PASS (vitest 81/81, cargo 123/123, clippy/fmt, `tauri build` EXIT=0), QA matrix 45 PASS/PASS(code) · 5 PARTIAL · 1 NOT RUN · 1 BLOCKED-by-design · 0 FAIL; review nội bộ 3 vòng (`PHASE_09_HANDOFF.md` §14).
- Quy trình repo: user đã bỏ bước reviewer ngoài (ghi nhận 2026-09-22, `docs/HANDOFF.md` §5) — ACCEPT thực hiện theo mô hình "review N vòng" của user; **chốt ACCEPT/REJECT cuối cho Phase 10 vẫn thuộc user/Codex Reviewer**. Không có bằng chứng ACCEPT bên ngoài nào được giả định.
- Release blockers còn mở (từ P09, không đổi bởi P10 config): R-1 live download journey + 13 mục manual Windows — cần người thật/VM sạch.

## Quyết định (khóa trước khi code)

| Hạng mục | Quyết định | Cơ sở |
|---|---|---|
| Product name | `Manga Downloader 1989` (không đổi) | `productName`/title đã ACCEPT từ P01–08 |
| Bundle identifier | `com.manga1989.downloader` (**không đổi**) | identifier quyết định app-data path; đổi = mất library của user hiện hữu |
| Target | Windows **x64** duy nhất | máy dev + toàn bộ evidence là x64; ARM64 = work item riêng |
| Bundle format | **NSIS only** (`targets: ["nsis"]`) | plan §3: một format có chủ đích; MSI từng build P09-only, không còn là deliverable |
| Installer scope | `installMode: currentUser` (per-user) | không admin, không system-wide writes; khớp local-first |
| Publisher metadata | `Manga Downloader 1989` | chỉ giá trị truthfully known — không tổ chức tưởng tượng |
| Version | **1.0.0** — first release | `git tag -l` rỗng, không prior release; authoritative = cả 3 file đồng bộ, enforce bằng `scripts/check-version.mjs` (`npm run check:version`) fail on drift |
| App icon | sinh deterministic từ `scripts/make-icon.mjs` → `npx tauri icon` (full set, thay placeholder 16×16 — pending item từ P04) | Dark Purple tokens; provenance nằm trong repo |
| WebView2 | giữ default `downloadBootstrapper` | máy Win11 có sẵn runtime; offline-first install **không** được yêu cầu → offlineInstaller ghi là option tương lai |
| Code signing | **KHÔNG** — chưa có certificate/dịch vụ được chủ sở hữu ủy quyền | plan §2: không tự chế cert; mọi test signing = BLOCKED/N-A; SmartScreen caveat ghi trong runbook |
| Updater | **KHÔNG bật** — không endpoint, không key custody, không hạ tầng test N→N+1 (branch B) | manual upgrade bằng installer mới là đường được tài liệu hóa; không placeholder key/URL |
| CI | `ci.yml` (PR/branch: gates + build, artifacts staging, **không secrets**) + `release-windows.yml` (tag/dispatch → environment `release-staging` manual approval → artifact + manifest; **không bao giờ auto-publish**) | plan §8 |
| Distribution | local staging artifact + manifest SHA-256; publication destination = owner-approved, hiện **chưa tồn tại** → trạng thái cuối `RELEASE CANDIDATE READY — PUBLICATION BLOCKED` | plan §10/P10-10 |

## Identity/audit facts đã xác minh

- `git tag -l`: rỗng; GitHub repo upstream `GiaHan1907/Manga-Download-v2` — **không có release nào từng publish**.
- User data: `%APPDATA%\com.manga1989.downloader` (DB `manga-downloader-1989.db`, `library/`, `cache/`, `covers/`) — NSIS uninstall không đụng app-data (mặc định Tauri không xóa data) → kiểm bằng test matrix T10-24.
- Migration chain 0001–0005 forward-only; schema 5.
- Capabilities `default.json`: core:default + 5 window perms — **không đổi** cho release (rule §2.10).
- CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost blob:` — không đổi.
- Version hiển thị trong app (`app_get_info`) lấy `CARGO_PKG_VERSION` → nay 1.0.0, khớp About dialog.

## Trạng thái máy build

Windows 11 Pro x64 26200 · Node v24.18.0 · rustc 1.98.1 stable-msvc · @tauri-apps/cli 2.x · WiX đã download ở build P09 (NSIS không cần WiX).
