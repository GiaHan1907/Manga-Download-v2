# Phase 10 — Release Runbook

## Trạng thái hiện tại

`RC_TESTED (build-level)` → chờ **Codex Reviewer ACCEPT** trên commit RC → **AWAIT_OWNER_RELEASE_APPROVAL**. Agent không push/tag/publish (rule P10-10).

## Build lại RC từ source đã review

```powershell
npm ci
npm run check:version        # 1.0.0 ở package.json + tauri.conf + Cargo.toml + Cargo.lock
npm run lint; npm run typecheck; npm run test; npm run build
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cd ..
npm run tauri build -- --bundles nsis
# artifact: src-tauri\target\release\bundle\nsis\Manga Downloader 1989_1.0.0_x64-setup.exe
Get-FileHash src-tauri\target\release\bundle\nsis\*.exe -Algorithm SHA256
```

**Quy tắc hash:** hash tính trên FINAL bytes. Nếu bật signing sau này: sign TRƯỚC, hash SAU (T10-36). Build lại = hash mới = phải re-verify/re-review.

## Publish (CHỈ khi owner duyệt + Release Engineer thực hiện)

1. Owner xác nhận: version 1.0.0, destination (repo GitHub riêng/private release…), phạm vi (nội bộ hay công khai), có/không cert.
2. Tạo tag `v1.0.0` từ commit đã ACCEPT; push tag → `release-windows.yml` chạy trong environment `release-staging` (manual approval) → staging artifact + manifest.
3. Release Engineer đối chiếu hash remote ↔ manifest đã review; upload installer + `ARTIFACT_MANIFEST.md` + `RELEASE_NOTES.md` tới destination đã duyệt.
4. Thông báo kênh nhận hash chính thức để người dùng verify.

## Upgrade cho người dùng đang cài bản cũ

Không có auto-update (tắt có chủ đích — `SECURITY_SIGNING_UPDATER.md`). Đường hỗ trợ: **tải installer mới nhất → cài đè**. Cùng identifier ⇒ giữ nguyên `%APPDATA%\com.manga1989.downloader` (DB + library + cache). DB migration forward-only (0001→0005+); **không downgrade** DB nếu cài bản app cũ hơn — khôi phục bằng backup.

## Backup / Recovery

- **Backup đúng cách:** đóng hẳn app (process thoát — checkpoint đã durable) → copy toàn bộ `%APPDATA%\com.manga1989.downloader` (DB + `library\`). Không copy DB khi app đang ghi.
- **Restore:** đóng app → ghi đè thư mục backup → mở lại; schema cũ hơn sẽ tự migrate forward.
- **Cài thất bán/hỏng giữa chừng:** gỡ từ Settings → Apps, cài lại bằng file đã verify hash; dữ liệu người dùng không bị hai bước này đụng tới.
- **Thiếu WebView2 (máy offline):** cài Runtime từ Microsoft trước, hoặc cân nhắc build `webviewInstallMode: offlineInstaller` khi có yêu cầu phân phối offline thật (hiện chưa phải deliverable).
- **App không khởi động sau upgrade:** thu thập log stderr (`manga-downloader-1989.exe` từ console), kiểm DB integrity qua lần mở kế tiếp (migrations + `PRAGMA foreign_keys` đã bật per-connection); khôi phục từ backup là đường được hỗ trợ.

## Việc định kỳ mỗi release

- Update `version` 3 nơi + `npm run check:version` (script fail nếu lệch).
- Icon: `node scripts/make-icon.mjs && cd src-tauri && npx tauri icon ..\assets\app-icon-1024.png`.
- Chạy lại toàn bộ gates + checklist `WINDOWS_INSTALL_UPGRADE_TEST.md` trên VM sạch; cập nhật matrix + manifest.
