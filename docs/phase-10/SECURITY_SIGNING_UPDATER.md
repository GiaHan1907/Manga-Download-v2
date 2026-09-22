# Phase 10 — Signing & Updater Policy (P10-04 / P10-05)

## Windows Authenticode signing — **KHẢ DỤNG: KHÔNG** (trạng thái thật)

- Không tồn tại certificate Authenticode / EV / signing service nào được chủ dự án cung cấp. Plan §2 rule 7/§3: không tự chế cert tự ký để giả publisher → **mọi test signing (T10-29) = BLOCKED**, artifact được dán nhãn **UNSIGNED** nhất quán trong manifest, notes, runbook.
- Hệ quả được khai báo thẳng trong tài liệu người dùng: Windows SmartScreen có thể cảnh báo "unknown publisher" với installer/chưa có reputation — đây là hành vi bình thường của binary chưa ký, **không** phải bằng chứng lỗi app, và **không** có tuyên bố "không có prompt SmartScreen" ở đâu cả.
- Khi owner cấp signer trong tương lai: cấu hình qua `bundle.windows.certificateThumbprint` + `timestampUrl` (hoặc `signCommand`) chạy trong protected release job với secret store; KHÔNG BAO GIỜ trong PR CI; hash manifest phải tính **sau** bước ký (T10-36).

## Updater Tauri — **TẮT có chủ đích** (branch B của plan)

Điều kiện bật tồn tại đồng thời: endpoint HTTPS thật + public key thật + private key custody trong signing process + test N→N+1 trên VM. **Cả bốn đều chưa có.** Do đó:

- Không thêm `tauri-plugin-updater`, không tạo key giả, không đặt endpoint placeholder (rule §2.9).
- UI/Settings không có mục "Check for updates" nào chết — kiểm tra: không tồn tại đường dẫn updater nào trong code (grep `updater` = 0 kết quả production).
- **Đường upgrade được hỗ trợ và kiểm chứng: cài đè** — chạy installer mới nhất lên trên phiên bản cũ (cùng identifier `com.manga1989.downloader` → cùng app-data; DB forward-only migration 0001→0005 đã test ở P09). Runbook: `RELEASE_RUNBOOK.md` §Upgrade.
- Phân biệt rõ (rule §2.8): Authenticode = danh tính publisher; updater signature = xác thực payload update. Một cái không thay thế cái kia; hiện cả hai đều không bật.
- Hành vi khi update tắt: app chạy offline hoàn toàn bình thường (local-first, invariant P08).

## Upstream key/secret hygiene đã kiểm

- Repo không chứa file key/secret nào (`*.pem`, `*.key`, `*_key`, token mẫu) — quét thủ công + CI không có secret nào được tham chiếu trong 2 workflow (`grep secrets.` = 0).
- Log app không in path/credentials (P09 S-2 đã sanitize); notification body chỉ chứa title metadata.

## Việc còn mở (giao owner khi quyết định phát hành công khai)

1. Chọn kênh phân phối (GitHub Release private/repo riêng / internal share) → tạo environment `release-staging` + protection reviewers.
2. Nếu phân phối công khai: cân nhắc cert (AVC/OV) + quy trình custody; bật `release-windows.yml` signing step; tính lại hash sau ký.
3. Nếu cần auto-update: lập kế hoạch riêng (key custody, endpoint, migration test N→N+1) — không phải việc còn sót của Phase 10.
