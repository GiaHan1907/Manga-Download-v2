# Phase 05 Source Decision — user-selected sources

**ĐỔI CHÍNH SÁCH (user chỉ đạo, 2026-09-22):** gate "cần sự cho phép của nguồn" trong brief Phase 05 **được user (chủ dự án) gỡ bỏ** — nguồn do người dùng nhập URL chọn. App chuyển sang **adapter Madara generic + registry động**: người dùng tự thêm nguồn (tên + URL) trong Settings → Sources. Giữ nguyên các ràng buộc kỹ thuật: HTTPS-only, exact-host, SSRF guard, không lệnh fetch tùy ý, tuần tự + timeout, không bypass login/DRM/paywall (không có chức năng đó). **Cơ sở sử dụng (khai báo đã hiệu chỉnh của chủ dự án, 2026-09-22):** KHÔNG sở hữu site; được cấp quyền tải ảnh từ site. Bằng chứng hỗ trợ: chính trang truyện nhúng nút “Tải trọn bộ … về máy” (tính năng do site cung cấp cho người đọc). Chủ dự án đã chỉ đạo gỡ gate “cần sự cho phép của nguồn” và cho phép tích hợp theo lựa chọn của người nhập URL. App chỉ dùng các endpoint công khai mà frontend của chính site gọi (ajax tìm kiếm, đọc trang HTML thường), không bypass login/paywall/DRM, không vượt hạn mức truy cập; throttle ≥3s/request.

## Kết quả reverse-engineering `truyentuoitho.com` (2026-09-22, bằng chứng sống)

- Stack: WordPress 7.0.5 + plugin `madara-core` + theme `madara`/child `madara-child-mtp` (có phần WooCommerce + Google-Drive-PDF).
- **Search (đã test thành công, HTTP 200):**
  `POST /wp-admin/admin-ajax.php` với form `action=wp-manga-search-manga`, `title=<query>`;
  headers bắt buộc thực tế: `X-Requested-With: XMLHttpRequest` + `Referer` + UA trình duyệt;
  response: `{"success":true,"data":[{"title","url","type"}]}` — slug lấy từ `url` (`/manga/<slug>/`).
- Route truyện: `/manga/<slug>/` (post id trong `body.postid-N`, `data-postid`). HTML truyện chỉ nhả 3 chapter gần nhất + mới nhất (`listing-chapters_wrap > ul.main.version-chap`, nút `.wp-manga-ajax-button` phân trang chapter qua `wp-manga-nav`/`wp-manga-query-vars`).
- Route chương: `/manga/<slug>/tap-<n>/`; nội dung chương theo dom-hints của site: `.reading-content img.wp-manga-chapter-img` (**chỉ dùng ở Phase 06 — Phase 05 không đọc ảnh**).
- **WAF/điều tốc:** request dồn hoặc cách ~12s bị `connection reset`; cooldown ≥30-40s mới ổn → adapter Rust: 1 req/phiên bản tối thiểu 3s giữa các request, UA cố định, không retry tự động quá 1 lần.
- Nhận dạng URL của adapter: host `truyentuoitho.com` (+ `www.`), path khớp `^/manga/[^/]+(/[^/]+)?/?$` — chương con phần `tap-*` (resolve về manga cha).


---

(Phần ghi MangaDex bên dưới giữ nguyên như lựa chọn built-in thứ hai:)


**Ngày quyết định:** 2026-09-22 · **Trạng thái live-verification: BLOCKED-BY-NETWORK (bằng chứng dưới)**

## Provider: MangaDex REST API (`api.mangadex.org`)

| Tiêu chí | Chi tiết |
|---|---|
| Cơ sở cho phép | API REST công khai **được MangaDex chủ ý cung cấp cho client bên thứ ba**, có tài liệu tại docs.mangadex.org; không cần auth cho đọc metadata; không cào HTML, không bypass login/paywall/DRK/anti-bot |
| Operations dùng | `GET /manga?title=&limit=&offset=` (search), `GET /manga/{id}` (detail), `GET /manga/{id}/feed` (chapter list), attribution qua relationship `author`/`artist`; cover chỉ lưu **fileName reference**, không fetch ảnh |
| Identity | `source_manga_id` = MangaDex UUID (`/title/{uuid}`); `source_chapter_id` = chapter relationship UUID — cả hai ổn định vĩnh viễn theo thiết kế API ✓ yêu cầu §2.4 |
| Canonical URL | `https://mangadex.org/title/{uuid}` — host ownership exact-match, không substring |
| Host allowlist | đúng `api.mangadex.org` (HTTPS, port 443 mặc định). `mangadex.org` chỉ dùng làm **pattern nhận diện URL**, không phải endpoint fetch |
| Rate policy | Tuân thủ guide chính thức: ≤ ~5 req/s; build này **tuần tự 1 request/version**, timeout 15s, redirect tối đa 3 hop tự thẩm định từng hop, User-Agent định danh app, honor `Retry-After` (không tự retry hung hăng) |
| Giới hạn đã biết | Không có chapter số (specials → `number=NULL`, order theo API trả về); hiatus → status `unknown`; description/author có thể null → UI phải chịu được |
| Không làm trong Phase 05 | KHÔNG gọi `/at-home`/image manifest (Phase 06), không expose URL ảnh gốc qua IPC, không tải file |

## Live verification — BLOCKED (không quy chụp adapter lỗi)

Bằng chứng từ máy phát triển (Windows 11, Git Bash, 2026-09-22):

```
nslookup api.mangadex.org → 45.129.229.1/.2 (đúng MangaDex LB)
curl https://api.mangadex.org/ → "Recv failure: Connection was reset" (TLS handshake bị reset)
curl https://api.github.com/   → 200   (outbound chung OK)
```

→ Mạng nội bộ/ISP **censor** Mangadex ở tầng kết nối; không phải API từ chối. Hệ quả trung thực:
- Adapter + toàn bộ test chạy trên **mock transport + fixture JSON** (được brief cho phép: "do not require unrestricted internet").
- Smoke live: **NOT RUN / BLOCKED** — hướng dẫn re-verify đã ghi ở `test-results.md`; **Phase 05 KHÔNG được tự tuyên ACCEPT toàn phần** cho tới khi có một lần live search hoạt động trên mạng không bị chặn.

## Fixture adapter (test-only)

`fixture-demo`: deterministic catalog nội bộ (title/ID/chapter cố định, có trang malformed, timeout giả lập, error 404/429) — **không nằm trong registry production**, registry của app chỉ gồm adapter đã kiểm duyệt; service tests inject registry riêng.

## Ghi chú ngày 2026-09-22 (phiên điều tra)

- Sau chuỗi probe (manga page OK, search 200, rồi WAF siết: mọi endpoint trả connection-reset ≥4 phút) → **tạm dừng live probe**; chính sách adapter: ≥3s/request, 1 attempt, backoff khi bị reset, không retry vòng lặp.
- Chapter list **không có trong static HTML** (fork này chỉ nhả 3 chương). Adapter Madara sẽ: (A) parse `ul.main.version-chap li.wp-manga-chapter a` nếu fork khác có sẵn; (B) phân trang `?style=list` khi host mở; (C) **endpoint do chủ site cung cấp** — đang chờ user xác nhận (họ sở hữu site, tra `madara-core` PHP 5 phút là ra action/params đúng).
- Live-verification TTT: search ✅ (bằng chứng 200); detail ✅ (HTML đã lưu phân tích); chapters ⏳ chờ endpoint.
