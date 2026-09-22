# Phase 05 Test Results — bằng chứng THẬT (T05-01..T05-20)

**Môi trường:** Windows 11 x64 (10.0.26200), rustc/cargo 198.1 MSVC (`1.98.1`), Node v24.18, Git Bash. Mọi test DB Rust chạy trên tempdir cô lập; dev smoke dùng DB thật `%APPDATA%\com.manga1989.downloader\` (bản copy chỉ-đọc để kiểm tra).

**Gate tổng (chạy ngày 2026-09-22):** lint 0 lỗi · typecheck pass · vitest **62/62** (6 files) · vite build pass (365.86 kB JS) · cargo test **45/45** · `cargo clippy --all-targets -- -D warnings` sạch · `tauri dev` smoke **EXIT=0** · `tauri build` (packaging release) **NOT RUN** — gate bắt buộc trước release, cần user cho phép.

## Ma trận

| ID | Nội dung theo brief | Bằng chứng | Kết quả |
|---|---|---|---|
| T05-01 | Launch mới sau DB Phase 04; hành vi cũ không đổi | Dev smoke trên DB THẬT: `_sqlx_migrations` = [(1 init),(2 domain),(3 source config)] — upgrade forward-only chạy trên file đã có `app_settings`; vitest phase03/04 regression xanh; cargo db tests xanh | PASS |
| T05-02 | Load registry sources + enabled thật | `discovery_service::tests::t05_02_source_view_defaults_to_enabled_and_flags_builtin`; vitest `keeps the demo sandbox section separate from the real registry` | PASS |
| T05-03 | Search 1 nguồn, identity ổn định | `madara_generic::tests::search_maps_ajax_results_to_slugs` (slug = sourceMangaId, id = `madara-<host>`); `mangadex::tests::search_normalizes_entries_with_identity_and_cover_key`; vitest live search | PASS |
| T05-04 | Search All, 1 nguồn lỗi | `t05_04_search_isolates_provider_errors_and_rejects_unknown_ids` (SOURCE_NETWORK/SOURCE_DISABLED/VALIDATION_ERROR gắn đúng source, nguồn khác vẫn trả kết quả); vitest `shows attributed results alongside per-source errors instead of a fake empty state` | PASS |
| T05-05 | Mọi nguồn được chọn đều lỗi | Backend: items rỗng + sourceErrors đầy đủ, KHÔNG phải empty-success (test như T05-04; frontend ẩn EmptyState khi có errors). Hạn chế ghi thật: danh sách lỗi có thông điệp Retry ngữ cảnh, nhưng không có nút Retry riêng khi chỉ lỗi từng nguồn — UI bảo đổi query/bật nguồn | PASS (kèm hạn chế) |
| T05-06 | Query đổi nhanh / toggle nguồn | `useAsyncData` alive-flag (response cũ không ghi đè — cơ chế từ Phase 03, còn chạy xanh); disabled source không bị gọi (assert trong t05_04). Nhấp nhanh thủ công trên Windows: NOT RUN (chờ user) | PASS (headless) |
| T05-07 | URL live được hỗ trợ → preview + chapters | Endpoint search TTT verified LIVE khi recon (ghi trong `source-decision.md`); route/parse verified bằng fixture tests. Trong-app live TTT (search/preview/chapter-list) **PENDING** — WAF site reset request dồn, cần người bấm chậm qua UI; MangaDex live **BLOCKED** (mạng reset TLS tới api.mangadex.org — bằng chứng trong source-decision) | PARTIAL (có giải thích) |
| T05-08 | URL malformed / không thuộc nguồn / nguồn tắt | `resolve_url_previews_and_links_saved_manga` (INVALID_URL vs UNSUPPORTED_SOURCE phân biệt, URL lạ KHÔNG bị fetch); disabled → SOURCE_DISABLED (t05_04); vitest `surfaces the structured rejection code` | PASS |
| T05-09 | URL có credential, non-HTTPS, IP riêng, subdomain giả | `http::tests::rejects_credentials_and_ip_literals_and_private_targets`, `rejects_non_https_and_foreign_hosts` (exact-host, `api.mangadex.org.evil.test` fail); `madara_generic::tests::live_constructor_rejects_unsafe_base_urls` | PASS |
| T05-10 | Redirect tới host_blocked/private | `http::tests::redirect_targets_are_revalidated_before_the_next_hop` — mỗi hop được đưa lại qua `validate()` trước khi gửi (mới thêm vòng này) | PASS |
| T05-11 | 404/429/timeout/oversized/malformed | `script_404_429_500_parse_map_to_distinct_errors` (SourceError phân loại + retryable map đúng trong error.rs tests); size cap 2MB + content-length check CÓ TRONG CODE (`fetch_bytes`/`post_form_json`) nhưng chưa có unit test độc lập cho cap → ghi nhận PARTIAL ở hạng mục cap | PASS (cap: PARTIAL) |
| T05-12 | HTML/script trong title/description | vitest `renders hostile provider titles as inert text` — title `<img onerror=...>` render thành text, không DOM injection; mọi trường text render thô, không `dangerouslySetInnerHTML` trong source | PASS |
| T05-13 | Save cùng manga 2 lần | `t05_13_14_save_is_idempotent_and_refreshes_without_duplicates` — 1 record, mangaId ổn định, updated=true | PASS |
| T05-14 | Refresh chapters (trùng/lộn thứ tự/thiếu) | cùng test trên: 2 chapter, không bản ghi trùng; sort numbered-asc + unnumbered cuối trong `detail_parses_...chronological_chapters`; code không xóa chapter thiếu | PASS |
| T05-15 | Favorite/progress tồn tại trước refresh | `t05_15_save_preserves_favorites_and_reading_progress` — favorite còn, page_index giữ nguyên 7 | PASS |
| T05-16 | DB write lỗi giữa chừng → rollback | `t05_16_constraint_violation_rolls_back_entirely` — provider trả status vi phạm CHECK → Err, 0 dòng cả 3 bảng (kể sources INSERT đầu transaction) | PASS |
| T05-17 | Restart sau save | `t05_17_saved_metadata_survives_reopen` (init pool lại trên cùng file); `availableOffline:false` là hằng số DTO — không bao giờ suy ra từ metadata | PASS |
| T05-18 | Demo vs production không lẫn | toàn bộ suite phase03 xanh với Demo sandbox tách tab; vitest sources separation test; remote pages dùng `localServices.source` thật, fixture demo không bao giờ vào SQLite | PASS |
| T05-19 | IPC abuse / security audit | Bounds: query ≤200 ký tự, page 1..=50, sourceIds ≤10, validate_id mọi sourceId, URL ≤2048 (`commands/discovery.rs` + `domain`); `oversized_query_and_malformed_ids_rejected` (catalog_service) chạy cùng helper; không command nào nhận host/SQL/path/headers — capability + CSP không đổi so với Phase 04 (không wildcard, không http plugin). **Không** có test tự động riêng cho từng bound ở discovery commands (code review + reuse helper đã test) | PASS (headless, ghi chú) |
| T05-20 | Windows shell/NAV/Dark Purple regression | vitest 62/62 (chưa từng đổi theme/route cũ), dev smoke exit 0, build pass. Kiểm tra mắt trên Windows (scaling, drag/resize, bấm live TTT thật): **NOT RUN — chờ user** | PASS (headless) |

## Gate commands (lần chạy cuối)

```
npm run lint            0 errors
npm run typecheck       pass (tsc -b)
npx vitest run          62 passed (6 files)
npm run build           pass (tsc -b && vite build, 365.86 kB)
cargo fmt --all         clean
cargo test              45 passed
cargo clippy --all-targets -- -D warnings   clean
tauri dev smoke         EXIT=0, DB thật migrate 0003 thành công, sources bảng trống (chưa ai đăng ký)
```

## Sống (live) — trạng thái trung thực

- **truyentuoitho.com:** recipe search admin-ajax đã xác minh bằng probe HTTP thật (log trong `source-decision.md`). Trong-app flow chưa được bấm thủ công vì WAF reset khi request dồn nhanh; client đã pace ≥3s. Cần user xác nhận UI smoke khi mạng ổn định.
- **MangaDex:** BLOCKED tại mạng này (TLS reset tới api.mangadex.org, DNS OK). Adapter + tests xanh bằng fixture; không claim live pass.
- **Phase 06 (đã chốt với user):** tải thuần ảnh chương → nén `.cbz`. Phase 05 không tải gì; mọi nút download/reader vẫn disabled.
