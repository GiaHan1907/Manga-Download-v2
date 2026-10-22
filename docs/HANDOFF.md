# HANDOFF — trạng thái dự án cho AI/dev tiếp theo

**Cập nhật:** 2026-09-23 · **Branch:** `master` (local history, chưa từng force-push)
**Đọc trước khi làm việc:** `AGENTS.md` (quy tắc workflow), `README.md` (cấu trúc + gates), file này, rồi brief của phase sắp làm.

## 1. Đã hoàn thành

| Phase | Phạm vi | Self-review | Kết quả |
|---|---|---|---|
| 01 — Foundation | Tauri 2 + React 19 + Vite + SQLx/SQLite app-data, 2 IPC commands, CSP/capability chặt | `docs/phase-01/self-review.md` | ACCEPT (fix: bỏ serde_json unused, withGlobalTauri:false tường minh) |
| 02 — Design System & UI Shell | 11 tokens Dark Purple, custom titlebar + windowService, AppShell, primitives | `docs/phase-02/self-review.md` | ACCEPT (packaged `tauri build` pass — MSI+NSIS, checksums ghi trong file) |
| 04 — Database & Backend Core | Migration `0002_domain.sql`, Rust domain/services/repositories, 10 typed IPC commands; Settings/Library/Favorites = SQLite thật, phần còn lại sandbox demo có nhãn | `docs/phase-04/self-review.md` | ACCEPT (vòng 1; contract: `docs/IPC_CONTRACT.md`) |
| 03 — Core UI Screens (prototype) | 9 màn hình + typed mock services + fixture deterministic + test matrix T03-01→13 | `docs/phase-03/self-review.md` — 2 vòng: vòng 1 (`dd5434c`, `13081ee`), vòng 2 (`d9f7ee1`: F6 — Discover tôn trọng nguồn đã tắt, test hành vi end-to-end) | ACCEPT (fix: dialog focus trap, offline filter, task seq vào MockState, bộ lọc source ∩ enabled) |
| 05 — Source Engine & Manga Discovery | Rust `SourceAdapter`/`SourceRegistry` động + `GuardedHttp` (SSRF/redirect/pace), provider MangaDex + Madara generic do người dùng đăng ký (gate ủy quyền đã bỏ theo chỉ đạo user 2026-09-22 — xem `docs/phase-05/source-decision.md`), 9 IPC discovery commands, migration `0003_source_config`, Discover tab thật + `/remote/:sourceId/:sourceMangaId` + Sources add/remove/toggle; **không tải file nào** | `docs/phase-05/test-results.md` (T05-01..20) + self-review | round 1+2 (`docs/phase-05/self-review.md`) — sửa BLOCKING: FK sources trong save, madara slug/trailing slash, badge "Saved — chưa tải", test redirect + XSS | vòng 1+2 PASS; MangaDex live BLOCKED (mạng), TTT live pending manual |
| 06 — Download Engine & Queue | Migration `0004` (widen status CHECK + checkpoint cols, forward-only, test upgrade); `download_repo` state-machine/persist; `downloader/` engine: page manifest qua adapter (Madara `pages`; MangaDex KHÔNG declare — at-home host động xung đột exact-host), `GuardedHttp.get_bytes` cap 20MB, stream `.part`→validate→rename→CRC checkpoint, scheduler bounded (chapter 1–5 × image 1–8, pace/host), pause/resume/cancel/retry/backoff-co has budget, CBZ-Stored + folder export tự verify (ZIP-by-name, chống slip, atomic publish, adopt-if-verified), recovery 2 crash boundaries; 7 IPC commands + event `download-task-updated` (revision-guarded, coalesced); Downloads = manager THẬT, `/library/:id` chọn-chương-xếp-hàng, badge offline đã verify | `docs/phase-06/test-results.md` (T06-01..24, 83→84 cargo + 65 vitest) + `self-review.md` (F1–F9 BLOCKING đã sửa) | `docs/phase-06/{reconnaissance,download-design,integration-notes,handoff}.md` | engine fixture-green; **live CBZ = BLOCKED/PENDING-MANUAL** (T06-24b); `tauri build` NOT RUN |
| 07 — Manga Library & Offline Reader | Migration `0005` (forward-only ALTER: `mangas.last_read_at`, `chapters.local_size_bytes`, `reading_progress.scroll_offset/page_total` + index); library backend aggregate 1-SQL (counts/size/progress/has-missing, 4 sort × 4 filter × search escape), delete disk-first path-safe, reconcile non-destructive; reader backend: manifest thống nhất CBZ (tail-window EOCD + central-dir segment, STORE-only, sniff ≤12 trang)/image-folder, page bytes per-page qua `tauri::ipc::Response` (không base64-cả-file, không lộ path); UI Library chi tiết (grid/list, trạng thái thật, xóa confirm); Offline Reader `/read/:chapterId`: 3 mode + LTR/RTL + zoom + fullscreen(thật→fallback CSS) + keyboard, blob LRU revoke, prefetch; progress debounce 800ms + flush ranh giới chapter/unmount, "Đọc tiếp" THẬT ở Home/detail; 7 IPC commands (`docs/IPC_CONTRACT.md` §Phase 07) + 8 error codes `LibraryError` | `docs/phase-07/self-review.md` (implement: 3 BLOCKING — cbz whole-file read, sniff-all-pages, progress-write-mỗi-trang; **review 3 vòng: thêm 6 BLOCKING đã sửa** — R1 folder-page-size-cap, R2 stale-state-when-chapter-switch + vertical-scroll-sync/scrollOffset + favorite-filter semantics, R3 usePageImages pending-race + favorite-toggle catch; gates: cargo 101/101, vitest 76/76, `tauri dev` smoke nâng DB thật schema 4→5) | `docs/phase-07/{reconnaissance,library-reader-contract,integration-notes,handoff}.md` | ACCEPT sau 3 vòng; T07-24..32 Windows-manual NOT RUN; deflate CBZ defer |
| 09 — Testing, Security & Performance | Không đổi stack/migration/capability. +17 test Rust: 7 `reader/cbz.rs` adversarial (builder ZIP thủ công — file từng 0 test: deflate/traversal/declared-oversize/central-truncation/count-cap/non-image/empty), http DNS-guard fail-closed + pacing đo thật + classify/Retry-After + mock-size-cap, corrupt-settings-blob fallback, concurrent-writers busy_timeout, Unicode path slug; `perf.rs` tripwires: library 1k manga×10k chapter list **9.7 ms**/search **4.9 ms**, CBZ 300 trang open **18.9 ms**/page **0.12 ms**, checkpoint fsync 4 ms/write (chấp nhận có chủ đích — R-3); bug-fix testkit u8-overflow; log-sanitize (io::Error→kind); **`npm run tauri build` EXIT=0 — MSI+NSIS đầu tiên kể từ P04** + release exe smoke 12 s; npm audit 0 vuln; cargo audit triaged (rsa advisory không có trong Windows tree); QA matrix T09-01..52: 45 PASS/PASS(code) · 5 PARTIAL · 1 NOT RUN · 1 BLOCKED-by-design · 0 FAIL | `docs/phase-09/{QA_MATRIX,RISK_REGISTER,SECURITY_REVIEW,PERFORMANCE_BASELINE,WINDOWS_SMOKE,PHASE_09_HANDOFF}.md` + `evidence/` | **RC-ready mức automated**; blocker mở duy nhất: R-1 live journey + manual checklist 13 mục chờ user; chờ Codex Reviewer |
| 08 — Integration & Advanced Features | KHÔNG migration mới; audit + `integration-map.md`; invalidation bus 7-domain (hiện hành 6 — `progress` đã gỡ ở review toàn project 4 vòng vì không có bump/subscriber) + `AppEventBridge` cấp app (event `download-task-updated` → bump domain, snapshot luôn từ Rust; thêm event `tray-open-downloads`); Home 100% thật (`library_stats` + recent tasks); Favorites pagination; Downloads paging theo `total`; Command Palette Ctrl+K (local-only qua `library_list`, keyboard ↑↓/Enter/Esc, focus restore, XSS-inert, remote search chỉ khi user chọn); Settings production (`notificationsEnabled`, `closeToTray` validate Rust + `RuntimeFlags` sync; bỏ option PDF; dirty-save); storage: `storage_status` + `clear_regenerable_cache` (chỉ staging terminal/orphan + covers, chặn active, idempotent, startup pass) + policy không đổi-root; Windows: `tauri-plugin-notification` theo chapter-terminal (Rust-side — capabilities KHÔNG đổi), tray `tray-icon` Show/Downloads/Quit qua lifecycle, close-to-tray; demo routes (`/manga/:id`, `/reader/...`, tab Demo) gate `import.meta.env.DEV` — production không còn mock leakage | `docs/phase-08/test-results.md` (T08-01..36: PASS wiring/fixture; NOT IMPLEMENTED có chủ đích: clear-history, đổi root, open-folder; NOT RUN: manual Windows; BLOCKED: live T08-36) + `settings-and-storage-policy.md` + `self-review.md` (review 3 vòng: 9 BLOCKING đã sửa — tray non-fatal, event-flood→terminal-only bump, window.confirm→inline, palette Esc/Tab-trap, favorites error, filter empty-state, settings dirty kẹt, RuntimeFlags default mirror, +2 test) | `docs/phase-08/{integration-map,settings-and-storage-policy,test-results,PHASE_08_HANDOFF}.md`; vitest 81/81, cargo 106/106, `tauri dev` smoke ×2 với tray+plugin sạch lỗi |

| 10 — Windows Build & Production Release | Version **1.0.0** đồng bộ package.json/tauri.conf/Cargo.toml/Cargo.lock + `scripts/check-version.mjs` (drift → exit 1, đã chứng minh); icon set thật đầu tiên (`scripts/make-icon.mjs` deterministic → `npx tauri icon`, thay placeholder 16×16); bundle **NSIS-only per-user** (`installMode currentUser`, publisher truthfully = product name), WebView2 `downloadBootstrapper` documented; staging build EXIT=0 + exe smoke; signing policy **UNSIGNED** (không cert — T10-29 BLOCKED, không fake-publisher); updater **tắt có chủ đích** (branch B: không endpoint/key giả, manual upgrade = cài đè, runbook backup/recovery forward-only); 2 GitHub workflows config-only (`ci.yml` gates + staging artifact, `release-windows.yml` environment `release-staging`, **không bước publish**, chưa chạy trên GitHub thật); ma trận T10-01..40: 25 PASS (8 có điều kiện config/fixture) · 2 PARTIAL · 6 NOT RUN · 3 N/A · 4 BLOCKED · 0 FAIL | `docs/phase-10/{RELEASE_CONFIGURATION,SECURITY_SIGNING_UPDATER,RELEASE_NOTES,KNOWN_ISSUES,WINDOWS_INSTALL_UPGRADE_TEST,RELEASE_RUNBOOK,ARTIFACT_MANIFEST,RELEASE_TEST_MATRIX,PHASE_10_HANDOFF}.md` | **RC_TESTED — PUBLICATION BLOCKED**; agent không push/tag/publish; ACCEPT cuối thuộc user/Codex Reviewer |

**Gate hiện hành (2026-09-25, thêm feature đổi nơi lưu library — user yêu cầu):** version **1.0.0** đồng bộ 4 nguồn (`npm run check:version`) · lint 0 warning · typecheck · vitest **114/114** · vite build · cargo fmt/clippy(`-D warnings`)/test **176/176** · IPC registry **55 lệnh** (F2/F5: `clear_download_history`, `retry_all_failed_downloads`, `cancel_downloads_for_manga`; F1: `library_get_watch`, `library_set_watch`, `library_list_alerts`, `library_ack_alerts`, `watch_run_now`; F4: `app_list_read_history`, `app_clear_read_history` (schema 7); F3a đọc CBZ STORE+DEFLATE qua `flate2` (user-duyệt dependency), F3b `library_import_cbz` dùng `tauri-plugin-dialog` **chỉ phía Rust** (không thêm capability `dialog:`, renderer không thấy path) — `docs/IPC_CONTRACT.md` §F1/F3; thumbnail: `get_manga_cover` (`LibraryMangaDto`+`MangaDto` mang sourceId/sourceMangaId/coverKey → bìa cả Library grid, header truyện đã lưu, hero Đọc tiếp Home) — search Madara đổi sang trang HTML `?s=` (card có ảnh bìa + phân trang `paged`; 0 kết quả → fallback biến thể lưới `.page-item-detail` scoped trong 2 listing wrapper, chống sidebar popular), cover fetch qua adapter-guarded transport + cache `covers/` (regenerable), dùng chung gate require_enabled (nguồn tắt → SOURCE_DISABLED cả khi còn cache)). **Thư mục tải sạch (schema 9):** `mangas.library_dir` claim một-lần-trong-tx (slug không dấu latinize, va chạm slug → fallback token; ổn định qua đổi tên); file mới = `chapter-<number>.cbz` (1.5→`chapter-1-5`; không số→token); hàng đợi cũ giữ nguyên relpath đã lưu trong DB; import CBZ dùng chung layout. **Đổi nơi lưu (2026-09-25, user yêu cầu — thay policy 'không đổi root' của P08):** `get_download_root`/`choose_download_root`/`reset_download_root` — dialog thư mục chạy TRONG Rust (như F3b, KHÔNG thêm capability), validate abs/system/appdata-nesting, copy-then-verify từng byte rồi NTFS junction `appdata\library → <đã chọn>\library` (mọi consumer reader/reveal/import không phải đổi), link hỏng → `storage_status.rootLinkOk=false` + cảnh báo UI (không bao giờ hiện library trống giả), key settings `storage.downloadRoot.v1`, staging/cache ở lại app-data — `docs/IPC_CONTRACT.md` §Nơi lưu **Lưu ý RC hash:** artifact `2245f30e…a90674` tương ứng tree commit review-4 (`3f4eda2`) — tree hiện tại đã lớp UI polish + feature lên → **REBUILD 2026-09-25 (cây `9fe0d69`, gồm feature đổi nơi lưu): NSIS `Manga Downloader 1989_1.0.0_x64-setup.exe` — 4.784.353 bytes, SHA-256 `a91e08a42fd7ea7a…` (hash rút gọn; bản đầy đủ ghi lại khi phân phối — `sha256sum` trong bundle/nsis). Artifact `0.1.0` cũ đã đổi tên `STALE-*`. RELEASE_RUNBOOK/manifest chưa cập nhật hash mới — việc khi nào phân phối; publication vẫn BLOCKED**. **Trạng thái: `RC_TESTED — PUBLICATION BLOCKED`** (chờ Reviewer ACCEPT + owner approval; VM install checklist `docs/phase-10/WINDOWS_INSTALL_UPGRADE_TEST.md` chưa ai chạy).

### 1d. UI theo mockup 2026-09-29 — Trang chủ + Khám phá (user yêu cầu trực tiếp, không có brief phase)

**Theme:** Dark Purple toàn app (user ĐẢO quyết định "Paper & Ink một theme duy nhất" trước đó); palette trong `src/styles/tokens.css` lấy mẫu trực tiếp từ ảnh mockup, không tự đặt màu.

**Trang chủ:** nguồn là rail của chính TruyenQQ (`get_home_feed`) — hero "Đang đọc tiếp", hàng hot có nút `<`/`>` (6 card/hàng), rail "Mới cập nhật", cột "Bảng xếp hạng" 3 tab Ngày/Tháng/Tuần (`get_home_ranking`). Cột "Thao tác nhanh" từng làm theo mockup cũ đã bị user bác và thay bằng Bảng xếp hạng.

**Nguồn build sẵn:** TruyenQQ chuyển sang `truyenqqko.com` (engine khác: `/truyen-tranh/<slug>-<id>`, search `POST frontend/search/search`, cover shard `hinhhinh.com`, trang chương shard `truyenvua.com`, `/top-ngay|tuan|thang`); NetTruyen `nettruyen.gg` cần `Referer` + CDN `kcgsbok.com` — hotlink 403 làm hỏng cả thumbnail lẫn tải chương.

**Khám phá — Giai đoạn 1 (đã làm):** một màn hình thống nhất thay cho 3 tab — hero "Khám phá truyện" (ô tìm kiếm nằm TRONG hero theo mockup: icon + trường + nút "Tìm kiếm", dưới là pill lọc "Nguồn"), card "Thêm bằng URL" (trường + nút "Phân tích" cùng hàng; `LiveUrlImportPanel` → `resolve_manga_url`), "Đề xuất cho bạn" (6 card/hàng từ `featured`, nút `+` gọi `save_discovered_manga`), cột phải "Nguồn truyện được hỗ trợ" (`list_supported_sources` + `set_source_enabled`) kèm link "Quản lý nguồn truyện". Ảnh hero lấy từ bìa của truyện nổi bật đầu tiên — đó là ảnh THẬT duy nhất có, không dùng artwork trang trí. Thẻ "Chưa có truy vấn" đã bỏ (placeholder + nút disabled đã nói rõ cần ≥2 ký tự). Sandbox fixture demo KHÔNG còn là tab — chỉ còn ở route dev `?tab=demo|demo-url` (`import.meta.env.DEV`), production build không render.

**Chưa có trong Giai đoạn 1 (mockup vẽ nhưng cần backend — Giai đoạn 2):** pill "Thể loại" / "Trạng thái" / "Sắp xếp" (adapter `search` hiện chỉ nhận `query + page`), "Xem tất cả" cho hàng đề xuất (không có màn liệt kê toàn bộ), và mục "Tìm kiếm gần đây". Lọc thể loại/trạng thái làm client-side trên trang đã tải sẽ là bộ lọc giả (chỉ lọc ~20 kết quả đang hiện, không phải toàn bộ nguồn) nên không làm.

**Loại có chủ đích vì mockup bịa số liệu (dự án cấm UI giả):** số truyện per-thể-loại / per-trạng-thái ("Hành động 12.4K"), ★ rating, lượt xem, hạn mức "500 GB", và ví dụ URL `mangakalot.com` (nguồn chưa đăng ký). Dòng "dựa trên xu hướng và sở thích của bạn" viết lại thành "Những bộ đang nổi bật trên TruyenQQ — chưa có gợi ý theo sở thích cá nhân" vì không có mô hình gợi ý nào.

**Giai đoạn 2 — ĐÃ LÀM một phần (2026-09-30):**
- Màn **Bảng xếp hạng** (`/ranking`, mục sidebar mới) dùng `get_home_ranking` đã có — 3 tab Ngày/Tuần/Tháng, top 3 gắn vương miện, KHÔNG có lượt xem vì nguồn không công bố. Probe live 2026-09-30 xác nhận 3 kỳ THỰC SỰ khác danh sách (phần giao nhau day∩week 8/10, day∩month 7/10, week∩month 9/10 — cùng ngôi đầu nhưng khác từ hạng 2).
- **Tìm kiếm gần đây**: migration `0010_recent_search.sql` (schema 9→10) + bảng `recent_searches` (CHỈ lưu đúng câu truy vấn + timestamp — không kết quả, không sourceId), UPSERT theo query nên tìm lại không nhân bản, mỗi lần ghi cắt còn 10 dòng mới nhất. 4 lệnh `search_history_record|list|remove|clear`. Chip hiện dưới ô tìm kiếm; chỉ ghi khi người dùng THỰC SỰ bấm Tìm kiếm/chọn chip (không ghi từng keystroke).
- Màn **Lịch sử** (`/history`, mục sidebar mới) = tìm kiếm gần đây + chương đã đọc gần nhất (`app_list_read_history` đã có từ F4).
- **Bộ lọc danh mục thật**: `list_source_genres` (46 thể loại đọc từ chính trang nguồn, kèm nhãn tiếng Việt) + `browse_manga{genre,status,sort,page}`. Recon live (probe `#[ignore]`) xác nhận `the-loai/<slug>`, `truyen-dang-theo-doi`, `truyen-hoan-thanh`, `truyen-moi-cap-nhat` dùng chung lưới `ul.list_grid li` (42 thẻ/trang, phân trang `/trang-N`, trang 2 KHÔNG trùng trang 1). Pill "Thể loại" và "Trạng thái" nằm trong hàng filter của hero. **Vì sao là mode riêng:** nguồn không có route nào lọc kèm từ khóa, nên khi chọn bộ lọc thì ô tìm kiếm bị TẮT kèm ghi chú, thay vì giả vờ lọc trên ~20 kết quả đang hiển thị (đó sẽ là bộ lọc giả).
- **Pill "Sắp xếp" (2026-09-30, probe `live_builtin_qq_sort_probe`):** route thể loại có sẵn `select` với 6 giá trị `?sort=0..5` (Ngày đăng ↑↓, Ngày cập nhật ↑↓, Lượt xem ↑↓). Bằng chứng live: 6/6 cặp sort trả về THỨ TỰ KHÁC NHAU trên cùng một thể loại (trùng hợp chỉ 0–6/42 id), `?sort=` sống qua `/trang-2` (overlap 0). Probe còn bác bỏ quyết định cũ "genre XORK status": trang thể loại THU hẹp được bằng `?status=0|2` (42 thẻ, khác hoàn toàn trang thể loại không lọc). Ngược lại cả 3 route trạng thái IGNORE `?sort=` (`same_order=true`), nên sort không kèm genre bị từ chối ở service (`VALIDATION_ERROR`) và ở adapter (`UnsupportedOperation`) — pill Sắp xếp TẮT cho tới khi chọn thể loại, kèm ghi chú lý do.
- **Deep link danh mục**: `/discover?genre=&status=&sort=` (giá trị ngoài tập đóng bị bỏ, quay về mode từ khóa). Home nối lại 3 link "Xem tất cả": "Truyện đang hot" và panel "Bảng xếp hạng" → `/ranking`, "Cập nhật gần đây" → `/discover?status=new`. "Nguồn truyện" thành mục sidebar cấp 1 (`/settings/sources`, route `exact` để không sáng cùng "Cài đặt").
- **Chưa làm:** bảng đếm truyện per-thể-loại/per-trạng-thái ở cột phải (mockup vẽ số bịa), pill "Sắp xếp" cho trang trạng thái (route nguồn không hỗ trợ).

### 1e. Thư viện theo mockup — 2026-09-30 (user: "Sửa lại giao diện Thư Viện giống hệt mockup")

Mockup mới (`Desktop\ChatGPT Image Sep 30, 2026, 11_02_45 AM.png`) vẽ Thư viện có dữ liệu CHƯA TỒN TẠI trong app, nên backend phải làm trước (user chọn "Đủ 100% mockup (làm backend)"):

- **migration `0011_manga_genres.sql`** (schema 10→11): bảng `manga_genres(manga_id, genre)` PK ghép, index theo genre. Thể loại được ghi trong ĐÚNG transaction `discovery_service::save` (xoá-and-ghi lại toàn bộ mỗi lần lưu/nhập lại metadata, nên refresh không để lại tag cũ). Nguồn đã có `RemoteMangaDetail.genres` (adapter cắt 12) — trước đây bị vứt.
- `library_list`: thêm `genres[]` + `description` mỗi dòng và tham số `genre` (EXISTS, không nhân bản dòng). Ba filter mới `reading` / `completed` / `missing` để đúng 6 ô "Bộ lọc nhanh" đều bấm được thật.
- `library_stats` mở rộng: `totalChapters`, `downloadedMangas` (số TRUYỆN có ≥1 chương offline — khác số chương), `reading`, `completed`, `paused`, `missing`. Test `t11_quick_filters_match_the_counts_they_display` buộc mỗi con số hiển thị = tổng của filter cùng tên, chống "đếm một đường, lọc một nẻo".
- **`library_genres`** (lệnh mới): thể loại có trong thư viện + số truyện của chính app.
- **Dung lượng ổ thật**: `storage::volume_space()` gọi `GetDiskFreeSpaceExW` qua FFI tự khai báo (`#[link(name="kernel32")]`) — KHÔNG thêm dependency mới. Đi theo junction nên số liệu đúng ổ người dùng chọn. `null` ngoài Windows/lỗi → UI bỏ thanh, không thay bằng quota bịa.

**Bỏ vì không có nguồn số liệu:** ★ rating (9.1/9.2 trong mockup), và "500 GB" cố định. "Tạm dừng" trong mockup là số CHƯƠNG trong hàng đợi nên hiển thị thành ghi chú chữ, không phải ô đếm truyện.

Layout: hero "Thư viện của bạn" (ảnh = bìa truyện đầu tiên đang đọc, không phải artwork), thanh tìm kiếm + chuyển Lưới/Danh sách, hàng pill Thể loại/Trạng thái/Sắp xếp + "Tổng N truyện", rail "Tiếp tục đọc" (3 card ngang có % tiến độ), lưới "Tất cả truyện" (6/hàng), "Mới thêm vào", cột phải 4 panel (Thống kê + thanh dung lượng, Bộ lọc nhanh, Đọc tiếp nổi bật có description, Hoạt động gần đây). Hai nút "Kiểm tra file"/"Kiểm tra chương mới" được GIỮ (chức năng thật) ở một hàng nhỏ trong hero.

### 1c. Feature plan (F2 + F5 + F1 + F4, 2026-09-23)

**F2** dọn lịch sử download: chỉ dòng terminal; `completed` còn file trên đĩa vẫn bị xóa DÒNG (file + `chapters.local_path` của Thư viện không đổi — sửa 2026-10-02: luật "giữ row còn artifact" khiến tính năng không bao giờ xóa được gì với lịch sử toàn mục hoàn tất); DELETE có guard status nên task vừa retry giữa chừng không bị xóa; `download_pages` cascade; không bao giờ xóa file. **F5** bulk retry mọi task lỗi (FIFO cap 200, đi qua guarded retry từng task — race-safe) + cancel mọi tác vụ chưa xong của một manga (worker nhận cancel flag hợp tác). UI: Dialog "Dọn lịch sử" (chọn trạng thái + tuổi) + "Thử lại tất cả lỗi" ở Downloads; "Hủy mọi tác vụ của truyện" ở LocalMangaPage; phản hồi qua toast. Test: 4 Rust (service+repo) + 3 vitest wrapper IPC. **F1 đã làm sớm hơn lịch v1.1:** watcher ~6h qua `manga_detail` adapter (pacing/GuardedHttp giữ nguyên), schema 6 `chapter_alerts`, per-manga opt-in, notification tổng + event `library-alerts`, filter "Có chương mới" + badge, banner chi tiết + "Kiểm tra chương mới" (rate-limit 60s). **F4 đã làm:** `read_history` (schema 7) UPSERT trong transaction save_progress khi đến 2 trang cuối; lệnh list/clear + section "Đã đọc gần đây" (list + xóa) trong LocalMangaPage. **F3 đã làm:** đọc CBZ STORE+DEFLATE (`flate2`) + import CBZ ngoài qua dialog Rust-side (`tauri-plugin-dialog`, không capability JS). **Đã làm thêm (2026-09-23, từ live probe R-1):** AJAX `ajax/chapters/` cho nguồn holder-pattern + allowlist `imageHosts` per-source (migration 8, tối đa 4 exact-host, chỉ dùng cho request ảnh). **Nút tải nhanh 5/10/toàn bộ chương chưa tải** (LocalMangaPage, theo thứ tự mục lục): frontend `queueMany` chia lô 100/chuỗi gọi `queue_downloads` (mirror `MAX_BATCH_SIZE` phía Rust) — **không đổi IPC/capability**. **Chưa làm:** F6 updater (owner-gate).

### 1b. UI Polish — plan `docs/plans/UI_IMPROVEMENT_PLAN.md` (đã triển khai C1→C5, 2026-09-23)

Reader immersive (auto-hide thông minh 2.5s + không nuốt focus, progress ring, 3 lamp theme persist localStorage, gutter double-page, focus-return Esc-fullscreen) · Downloads sống (sparkline tốc độ ≤30 mẫu, ETA từ page-progression, lỗi action inline tự clear theo revision) · Home phân tầng (hero Continue Reading + rail scroll-snap + skeleton layout) · a11y (tooltip nút disabled focus được, palette combobox `aria-activedescendant`) · toast stack thành công (max 3, 4s) + `ErrorState.onRetry`. **Zero** thay đổi IPC/CSP/capabilities/dependency. Nhật ký chi tiết + lệch plan: §7 của plan. Test mới `tests/uiPlan.test.tsx` (7 case); toàn bộ test cũ giữ nguyên, không sửa assert nào.

### 1a. Review toàn bộ project 4 vòng (2026-09-23) — tóm tắt fix

- **Vòng 1 — bất biến kiến trúc & security:** CSP bỏ `asset:`/`http://asset.localhost` dead-allowance (reader dùng IPC bytes + blob); `downloaded_chapters` chỉ probe path đã `validate_relpath` (chống DB-tampered traversal khi list); 3 ngoại lệ URL/path-input được khai báo tường minh trong `IPC_CONTRACT.md` (`resolve_manga_url`, `add_manga_source`, `outputRelpath` display-only). Quét sạch: network chỉ trong `http.rs`, không shell/process/opener, SQL động whitelist, frontend không `dangerouslySetInnerHTML`/`eval`/`fetch`.
- **Vòng 2 — data layer + engine:** (BUG-1) shutdown app KHÔNG còn dùng flag cancel → worker exit non-terminal, recovery re-queue kèm checkpoint (trước: mọi download đang chạy bị ghi `cancelled` vĩnh viễn khi thoát app); (BUG-2) cancel được tôn trọng tới tận trước commit artifact (re-check sau write+verify CBZ; folder mode chỉ undo rename do chính run tạo), pause bị từ chối thẳng `VALIDATION_ERROR` trong `verifying|exporting` thay vì nhận-lờ-không-làm; (BUG-3) `verify_cbz` bound-check mọi offset từ central directory — file tiền tồn tại hostile (adopt path) trả lỗi thay vì panic worker; migration 0004 chạy SQL repaired tại runtime (`PRAGMA foreign_keys=OFF` bị SQLite bỏ qua trong transaction → CASCADE từng wipe `download_pages` khi upgrade 2→4; giữ checksum gốc, có test thuận + negative-control); write transactions → `BEGIN IMMEDIATE`; `has_missing` chỉ báo khi mất artifact THẬT (in-app delete không còn bị báo missing; page_count catalog được giữ); recovery sweep orphan `.cbz.part`/`.part-dir` trong library, bound theo task-token DB; `queue_batch` reuse task idle nay nhận format mới; `clear_dir_contents` fail-là-fail khi read lỗi.
- **Vòng 3 — frontend:** action table gương lifecycle Rust mới (verifying/exporting chỉ còn cancel); `useDownloadQueue` chống snapshot cũ ghi đè state mới (request-sequence + monotonic revision merge kể cả khi events đi trước); `usePageImages` revoke blob đến muộn sau unmount (thủng memory từng phiên đọc); SourcesPage demo sandbox gate `import.meta.env.DEV` (trước lộ cả trong production build); Library không còn refetch theo mock-version; search results invalidate khi sources thay đổi; domain `progress` bị gỡ (bus còn 6); xóa dead code (`ChapterRow`, `ContinueReading` demo, `useCatalogData`, exports chết trong `domain/download.ts`, `pageFromScroll`); label "Tiến độ mô phỏng" → truthful. Tests mới: `tests/reviewRound3.test.tsx` (4).
- **Vòng 4 — docs-vs-code:** mục này + toàn bộ hash/counts/CSP/row lifecycle trong README + docs/ được đối chiếu lại source; RC rebuild từ tree đã review, re-hash manifest.

## 2. Trạng thái "thật" từng phần (sau Phase 08)

- **Thật (Rust + SQLite + đĩa):** Settings (kèm notification/tray keys có consumer thật), Library/Favorites local (pagination), discovery Phase 05, download engine + history pagination Phase 06, Library chi tiết + Offline Reader + Continue Reading + Home stats Phase 07–08, storage status/cleanup, Windows notification + tray (code+smoke; thao tác thật = manual checklist P09).
- **Demo còn lại — CHỈ trong bản dev (`import.meta.env.DEV`):** tab Demo sandbox Discover, khối Demo sandbox trong Settings → Sources (gate bổ sung ở review 4 vòng), `/manga/:id`, `/reader/:mangaId/:chapterId` (mock `src/services/mock/`). Production build: các route này rơi về 404, Home/Topbar/Settings không còn fixture/mock leakage. (Ghi chú hygiene: mock vẫn được static-import trong bundle prod — chỉ là vấn đề size, không có đường thực thi từ UI production.)
- **Live download:** ĐÃ chứng minh thật trên máy user — TTT (`madara-truyentuoitho-com`) tải xong 5 chương Ninja Loạn Thị 188–234 trang/chương ngày 2026-09-25 (DB `download_tasks.status=completed`). MangaDex vẫn BLOCKED ở mạng dev.

### 2a. Sự cố TTT `SOURCE_PARSE_ERROR` — 2026-10-01 (bài học đắt, giữ lại đây)

User báo "không tải được chapter nào": 5 task fail `SOURCE_PARSE_ERROR` với `total_pages = 0`, retry vẫn fail. Chuỗi nguyên nhân theo thứ tự đã loại trừ:

1. **Không phải** markup nguồn đổi, **không phải** tải song song — probe live đọc đúng tap-9/tap-10 trả 234 trang; 5 chương × 3 vòng song song = 15/15 OK.
2. Nguyên nhân thật: `sources.image_hosts` của row built-in là **NULL** (row chỉ do bật/tắt, chưa từng sửa). `builtin_adapters` dịch NULL thành "không có host ảnh" → adapter mất đúng `img.resourcehub.shop` mà built-in khai báo → mọi trang bị coi là host lạ → `Parse`.
3. Probe ở bước 1 KHÔNG tái hiện được vì nó truyền tay `TTT_IMAGE_HOSTS` thay vì dựng adapter qua registry — tức không đi đúng đường của app.
4. Sửa: `built_in_image_hosts()` — row có giá trị thì thắng, NULL nghĩa là "chưa chỉnh" nên dùng list shipped. Test thuận/đảo: `registry::tests` (2), `pages_on_a_host_the_source_never_registered_fail_the_chapter` (chứng minh host lạ vẫn giết chương), và probe live `live_registry_builtin_parses_ttt_chapter` dựng qua `SourceRegistry::from_rows` với row NULL → 231 trang thật.
5. Sửa đổi PHỤ được giữ: TTT đã bỏ class `wp-manga-chapter-img` nên chương đọc bằng nhánh loose `.reading-content img`; nhánh này nay BỎ QUA ảnh ngoài allowlist thay vì bỏ cả chương (một ad rotation không còn giết chương; vẫn không bao giờ request host lạ; vẫn fail nếu không còn ảnh hợp lệ). strict/boxed giữ hành vi nghiêm cũ.

**Bài học:** khi chẩn đoán lỗi của app, probe phải đi qua ĐÚNG đường dựng đối tượng của app (registry + row DB), không phải đường tương đương viết tay. "Probe chạy được" không phải bằng chứng app chạy được.

### 2b. Cầu dao mạng cho hàng đợi — 2026-10-01

Cùng ngày, DB cho thấy **45/47 chương fail trong 5 giây** (06:31:13–18) với `SOURCE_NETWORK`, `attempts=2`, trong khi `chapterConcurrency = 1`. Không có đường ghi hàng loạt nào trong code: một worker duy nhất đã lần lượt claim từng chương và mỗi lần connect bị RST trả lỗi trong ~50 ms, nên chỉ vài giây là đốt hết retry budget của cả truyện.

Sửa: `Coordinator::observe_network_failure` (gọi sau mỗi worker kết thúc) đếm chuỗi fail `SOURCE_NETWORK` **liên tiếp**; chuỗi khác (thành công hoặc lỗi nội dung như `VALIDATION_ERROR`) chứng tỏ nguồn vẫn trả lời nên reset. Đến ngưỡng 3 thì đóng `queue_paused` (đúng gate "Dừng toàn bộ" — người dùng bấm Tiếp tục để chạy tiếp) và **trả 3 chương vừa mất về `queued`** thay vì để `failed`. UI: hook re-read `get_download_queue_paused` khi thấy một row về `queued` mà không ai bấm retry, để marker "Đã dừng toàn bộ" không bị trễ.

Test: `network_breaker_stops_the_queue_and_returns_the_chapters_it_cost` (2 → chưa dừng, 3 → dừng + 5 row queued, 0 failed) và `network_breaker_only_counts_unreachable_source_failures` (xen một lỗi nội dung là chuỗi tính lại).

### 2c. TTT giấu ảnh trong script (chương mới) — 2026-10-01

`SOURCE_PARSE_ERROR` còn lại KHÔNG phải host allowlist cũng không phải quảng cáo. Probe live chỉ ra ranh giới rõ: tap-16…51 parse bình thường (205–243 trang), tap-56/61/64/65 thì `.reading-content` **không có `<img>` nào** — nguồn chuyển sang render trình đọc bằng JavaScript với payload mã hoá.

Đã loại trừ trước khi decrypt: `?style=list`, `?mode=list`, `?page-chap=1`, `?chapter_id=`, phân trang `/01/`, `wp-json/wp/v2/posts`, và 3 `admin-ajax` action — không route nào trả ảnh.

Thuật toán (đọc từ chính page, hai biến thể đang sống):
* `comic-app` (tap-61/65): `const d=atob('..')+atob('..')+…,s='<payload>'` → `{"images":["url",…]}`
* `c-w` (tap-56): `var k1='..',…,k4='..',d='<payload>'` với `gk(){atob(k1)+…}` → `{"i":[{"u":"url","w","h"},…]}`

Cả hai: `JSON.parse(XOR(base64(reverse(payload)), key))`. `encrypted_reader_images` vì thế KHÔNG hardcode gì: key = các literal `atob('..')`, nếu không có thì lấy `k1='..'…k4='..'`; payload = chuỗi base64 dài nhất trong script; đọc cả hai shape JSON. Biến thể mới không khớp → trả `None` → chương fail loudly `Parse`, không bao giờ tải nhầm.

Hai thứ bắt buộc phải sửa kèm, nếu không decrypt vẫn vô dụng:
1. Ảnh nằm ở Cloudflare Worker `truyentuoitho.anooymi.workers.dev` → phải có trong `TTT_IMAGE_HOSTS` (giờ là CSV hai host; `live()` validate TỪNG host nên probe phải tách CSV bằng `ttt_image_hosts()`).
2. Worker trả **403** nếu thiếu `Referer` → `SiteProfile::Madara` cũng được đặt referer = chính origin của nguồn (Dilib vẫn không, vì ảnh nằm ngay base host).

Bằng chứng live sau khi sửa: tap-56 = 243 trang, tap-61 = 242, tap-65 = 242, và trang đầu tải thật 300–356 KB (`live_builtin_ttt_encrypted_chapter_parses`). Test fixture: `reader_script` dựng lại đúng phép XOR/reverse/base64 cho cả hai shape.

**Rủi ro đã biết:** đây là lớp chống scraping do họ chủ ý thêm. Key, shape JSON, tên worker và quy tắc 403 đều có thể đổi; khi đó các chương mới sẽ lại `SOURCE_PARSE_ERROR` trong khi chương cũ vẫn chạy — đó là dấu hiệu cần probe lại trang, không phải lỗi DB hay host.

### 2d. Hàng đợi tải chạy lộn xộn — 2026-10-01

User: "trong hàng đợi không download theo thứ tự chapter mà download random". `claim_next` xếp `priority DESC, created_at ASC, id ASC`, nhưng `queue_batch` ghi **cùng một `created_at`** cho cả batch (DB user: 64 chương đều `2026-10-01T05:53:55.471Z`) → tiebreak rơi xuống `id` là UUID → thứ tự đúng nghĩa ngẫu nhiên. Sửa: nối `chapters` và tiebreak theo **đúng biểu thức `manga_repo::list_chapters`** (`number IS NULL, number, created_at, id`) để hàng đợi chạy giống danh sách chương user nhìn thấy; `retryable_failed_ids` cũng đổi theo cho khớp. Test `t06_claim_order_follows_chapter_number_within_one_submission` seed chương theo thứ tự 3-1-2 và chốt claim ra 1-2-3. Không cần migration — thứ tự tính lúc claim, nên hàng đợi 64 chương đang chờ của user tự vào lại đúng trật tự.

### 2e. Ngân sách retry dài hơn — 2026-10-05 (user: "Cho backoff dài hơn")

User bị `SOURCE_NETWORK` ("Không kết nối được nguồn (mạng/chặn theo khu vực)") đúng lúc hàng đợi chạy, nên mỗi chương chỉ có 3 lần thử cách nhau 5s/20s/80s — một lần DPI reset kéo dài vài phút là cả truyện chết `failed` trước khi mạng hồi.

Sửa trong `src-tauri/src/downloader/mod.rs`: `MAX_ATTEMPTS` 3 → **6**, `BACKOFF_CAP` 15 phút → **30 phút**, thang delay 5s → **15s** cơ sở (`base = 15 * 4^(attempt-1)`), được **15s / 1 phút / 4 phút / 16 phút / 30 phút**, tổng chờ ≈ **51 phút** cho một chương. Task id vẫn cộng jitter nhỏ để cả batch không retry cùng thời điểm.

Không đổi luật nào khác: chỉ lỗi *transient* mới được requeue, lỗi nội dung (parse/permission/validation) vẫn fail ngay, và cầu dao mạng §2b vẫn dừng hàng đợi sau 3 lần `SOURCE_NETWORK` liên tiếp — tức backoff dài không biến một nguồn chết thành một giờ retry âm thầm.

Test: `retry_budget_spans_about_an_hour` chốt hình thang (attempt 1 trong 15–17s, 2 = 60s, 4 = 960s, 5 = cap) và tổng tiền trong `[25 phút, 60 phút]`; `engine_tests` tham số theo `MAX_ATTEMPTS` nên vẫn xanh.

### 2f. "Lỗi chọn chỗ lưu" — tái liên kết khi junction hỏng — 2026-10-05

User kéo thư mục `library` ra khỏi `H:\My Drive\Truyện Tranh` (thư mục đã chọn làm nơi lưu) bằng Explorer. Hệ quả: `appdata\library` vẫn là junction trỏ vào `…\Truyện Tranh\library` **không còn tồn tại** → Settings báo đúng hai dòng "Không đọc được thư mục library" + "Link tới thư mục tùy chọn đang hỏng". Truyện không mất: toàn bộ 2 GB vẫn ở `H:\My Drive\library`.

Bug thật: chính thẻ Settings đó hướng user bấm "Chọn thư mục lưu…" để tái liên kết, và nút ấy trả **"Lỗi nội bộ không xác định."** Vì `relocate()` mở đầu bằng `lib_slot.canonicalize()?` — với junction treo, `canonicalize` lỗi → `AppError::Io` → wire `INTERNAL_ERROR`. Tức là con đường cứu dữ liệu duy nhất trong UI bị chết đúng lúc cần nó.

Sửa trong `services/storage_root.rs`:
1. `reachable_source()` trả `Option` — junction treo nghĩa là "không có gì để copy", không phải "lỗi không xác định".
2. Nguồn không khả dụng → **tái liên kết tại chỗ** vào `<thư mục chọn>\library` đang có dữ liệu (không copy, không nhân bản bản 2 GB).
3. Chốt an toàn: link hỏng mà nơi chọn **không có `library` nào** trong khi DB còn ghi nhận N chương đã tải → `VALIDATION_ERROR` nói rõ phải chọn thư mục ĐANG chứa `library` (VD: file ở `D:\Manga\library` thì chọn `D:\Manga`). Không chốt này thì một cú chọn sai sẽ biến thư viện thành "trống" ngay trước mắt user dù dữ liệu vẫn còn.
4. `reset_to_default()` cũng không còn `canonicalize()?`: không đọc được nguồn thì báo thẳng "không có gì để copy về, hãy tái liên kết trước" và **giữ nguyên** junction + settings.
5. UI: đổi câu hướng dẫn sang "chọn đúng thư mục ĐANG chứa thư mục `library`" (câu cũ "trỏ lại đúng thư mục cũ" sai khi user đã chủ ý di chuyển).

Test (Windows-only module, junction thật): `broken_link_relinks_the_folder_that_still_holds_the_library`, `broken_link_refuses_a_folder_holding_no_library` (kèm khẳng định không để lại `library` rỗng và junction cũ còn nguyên), `reset_with_a_broken_link_reports_the_real_problem`.

**Chi phí thật của bản sửa nằm ở một chi tiết không ai đoán trước:** bài test đầu tiên vẫn fail với *"Không tạo được liên kết thư mục: entity already exists"*. Hóa ra `junction::delete()` **im lặng trả Ok mà không xóa gì** khi junction treo (nó không stat được target), nên liên kết cũ vẫn nằm đó chặn `junction::create()`. Đã kiểm chứng bằng `cmd /c rmdir` trên một junction treo trong `.scratch`: RemoveDirectory xóa sạch link mà không đụng target. `remove_junction()` vì thế giờ thử `junction::delete` trước, còn nếu reparse point vẫn tồn tại thì fallback `std::fs::remove_dir` (và coi `NotFound` là thành công). **Bài học phụ:** thư viện tiện ích có thể "thành công giả" ở đúng trạng thái lỗi — luôn kiểm tra lại hiện trường sau khi nó trả `Ok`.

**Bài học:** mọi tính năng "tự chữa" phải được thử ở đúng trạng thái hỏng mà nó nhân danh. Ở đây driver của lỗi là `?` trên một phép `canonicalize` — một io error không tên tuổi lọt thẳng tới user dưới chữ "nội bộ".

### 2g. TTT sang shard ảnh thứ ba — và luật "subdomain thì thừa" — 2026-10-05

User báo `[SOURCE_PARSE_ERROR]`. DB: 7 chương của "Mankichi Đại Tướng Nhóc Con (Full)" fail, `total_pages = 0` cả bảy.

Probe live `live_builtin_ttt_zero_page_chapter_probe` (dựng qua `SourceRegistry::from_rows` với row NULL-host, đúng bài học 2026-10-01) cho thấy trang KHÔNG hề đổi markup kiểu mới: `.reading-content` có sẵn **324–346 `<img>`**, không class Madara, không script mã hóa — tất cả nằm trên **`img.truyentuoitho.com`**. Madara so khớp host **chính xác**, nên mọi trang bị xem là ảnh lạ → nhánh loose bỏ hết → manifest rỗng → `Parse`.

**Thủ phạm thật không phải danh sách host shipped.** `with_profile()` lọc mọi host trùng với base: `normalized == host || normalized.ends_with(".{host}")`, với lý luận "subdomain của base thì thừa, bỏ cho an toàn". Điều đó chỉ đúng ở nhánh TruyenQQ/Nettruyen (match theo domain), SAI ở nhánh Madara (match exact). Hệ quả: thêm `img.truyentuoitho.com` vào `TTT_IMAGE_HOSTS` vẫn vô dụng — nó bị loại trước khi kịp vào allowlist. Chính dòng probe in `allowed=[...]` thiếu host đã chỉ ra việc này; nếu chỉ đọc code sẽ tưởng danh sách shipped là nguồn sự thật.

Bốn sửa đổi, tất cả đều cần thiết:
1. Bỏ luật drop subdomain — chỉ còn loại host trùng tên base và trùng lặp trong danh sách.
2. `TTT_IMAGE_HOSTS` += `img.truyentuoitho.com` (giờ là 3 shard).
3. `MAX_PAGE_COUNT` 300 → **600**: bảy chương thật này dài 324–346 trang, tức **trần cũ tự tay từ chối cả bảy** — hai lỗi chồng lên nhau, sửa host mà không nâng trần thì vẫn nguyên triệu chứng.
4. Ba chỗ `if urls.len() >= MAX_PAGE_COUNT { break }` → `>`, để manifest có thể vượt trần một phần tử và đúng cái guard `urls.len() > MAX_PAGE_COUNT` (von trước tới giờ không bao giờ fire) mới chạy. Trước đó một chương quá dài bị **cắt cụt trong im lặng** và báo "Hoàn thành" với thiếu trang.

Bằng chứng live sau khi sửa: 7/7 chương trả 324–346 trang và ảnh đầu tiên của mỗi chương tải thật 56–157 KB (shard mới KHÔNG đòi Referer). Test đơn vị: `a_registered_subdomain_of_the_base_host_stays_allowed` (chốt đúng cái luật vừa gỡ), `page_box_images_on_the_truyentuoitho_shard_are_accepted`, `a_chapter_over_the_page_cap_fails_instead_of_being_cropped`.

**Bài học:** khi một adapter có HAI cách so khớp host (exact vs domain suffix), mọi luật "cái này thừa nên bỏ" phải được xét riêng cho từng cách. Và: một hằng số cấu hình bị lọc im lặng trên đường tới chỗ dùng — hãy in giá trị cuối cùng mà đối tượng thực sự mang, đừng tin danh sách khai báo.

### 2h. Built-in thứ năm: DocTruyen3Q (doctruyen3qhub.fit) — 2026-10-07

User: "Thêm trang build in : https://doctruyen3qhub.fit/". Đây là engine TỰ VIẾT (không WordPress, không Madara), nên nhận một `SiteProfile` riêng trong `madara_generic` — cùng pipeline đã kiểm chứng, chỉ khác grammar.

Đường live recon (agent khảo sát, HTML lưu ở `.scratch/d3q-*.html`):
* Truyện `/truyen-tranh/<slug>/<numericId>`; chương `/truyen-tranh/<slug>/chapter-<n>/<numericChapterId>`. **Id số là bắt buộc**: thiếu id → 404 thật, sai id → 302 tới `/error/404-manga` mà trang lỗi lại trả **HTTP 200** (soft-404 — nên không được tin status code).
* Tìm kiếm `GET /tim-truyen?keyword=…&page=N`, 36 card/trang, trang 2 không trùng trang 1. **Route này khớp slug ASCII không dấu**, nên `fold_search_query()` bỏ dấu tiếng Việt trước khi gửi — nếu không, mọi truy vấn kiểu "võ luyện" trả về 0 kết quả và nguồn trông như hỏng.
* Ảnh chương trong `div.list-image-detail div.page-chapter[id^='page_'] img[src]`, URL **protocol-relative**, trên các shard xoay số của CẢ MỘT DOMAIN (`s15.anhvip.xyz`, `s11.anhvip.xyz`, `s8.topcdnv1.art`) → dùng `allowed_domains` (khớp đuôi domain) chứ không phải exact-host. **Thiếu `Referer` là 403** với mọi shard (bìa thì không đòi).
* Bẫy 1: site tái dùng `.page-chapter` cho **quảng cáo ngay trên cùng domain ảnh**, nên bộ lọc buộc phải có `[id^='page_']`; bỏ điều kiện này là tải cả banner vào CBZ.
* Bẫy 2: dòng chương ĐẦU TIÊN là hàng ẩn `style="display:none"` trỏ domain KHÁC (`doctruyen3q.xyz`) — `chapter_slug_from_url` từ chối vì không phải host của nguồn, nên nó tự biến mất.

**Cách mã hóa id** (quyết định thiết kế quan trọng nhất): adapter chỉ truyền MỘT chuỗi identity, nên id số được ghép vào slug bằng dấu `.` — `naruto-full-color-edition.1263`, `chapter-700.9.2472187`. Dấu `.` hợp lệ trong `valid_slug`, id luôn toàn chữ số, và chương có số thập phân (`chapter-60.5`) nên tách/ghép theo **dấu `.` CUỐI cùng** (`split_id_slug`/`join_id_slug`). Số chương lấy từ `data-chapter` chứ không parse từ id (nếu parse sẽ ra `700.92472187`).

**Link chương không chứa id truyện → tự chữa (heal) ở bước refresh.** Route chương chỉ có slug truyện, mà site này 404 với mọi route thiếu id số, nên `chapter_from_url` trả `<slug>` thô và `manga_detail` chữa thành `<slug>.<id>` qua chính ô tìm kiếm của nguồn (`heal_story_id`) — `manga_detail` là bước DUY NHẤT đã gọi mạng trong luồng dán link, nên không thêm lần fetch nào mới. `discovery_service::save()` từ đó lưu theo `detail.summary.source_manga_id` (identity thật sự của bản chi tiết), không theo chuỗi người dùng đưa vào; nhờ vậy truyện dán bằng link chương và truyện tìm trong Discover là CÙNG một row. Hai luật chặn sai: chỉ nhận kết quả khớp **đúng nguyên slug** (khớp mờ sẽ ghi chương của truyện này vào truyện khác), và keyword phải đổi `-` thành **khoảng trắng** — probe live trả `Source(NotFound)` với `naruto-full-color-edition`, chỉ `naruto full color edition` mới tìm thấy.

Bằng chứng live của chính con đường user báo lỗi: `live_builtin_d3q_journey_probe` dán `…/truyen-tranh/naruto-full-color-edition/chapter-700.9/2472187` → `find_chapter_link` phân loại OK → `ensure_chapter_row` lưu truyện và trả row chương, row đó mang `source_manga_id = naruto-full-color-edition.1263`. Test đơn vị: `d3q_detail_heals_a_slug_without_the_numeric_id`, `d3q_detail_refuses_a_fuzzy_identity_match`, `d3q_resolve_url_maps_stories_directly_and_chapters_by_slug`.
`genres` đã đọc được (`li.category p.detail-info > a`, thêm vào selector dùng chung — Madara vẫn khớp `a[rel=tag]`); `description`/`author` vẫn `None` với MỌI profile HTML trong app (hạn chế có sẵn, không phải của riêng nguồn này).

Bằng chứng live (`live_builtin_d3q_journey_probe`, dựng qua registry + row y như app): search "naruto" → 2 card; detail → **707 chương**, 6 thể loại, cover key chuẩn; chương `chapter-700.9.2472187` → 24 trang, trang đầu tải thật 341 KB từ `s11.anhvip.xyz`; cover tải thật 21.976 KB→22 KB. Test đơn vị: 8 cái (`d3q_*` + `fold_search_query_*`), gồm cả fixture dựng đúng card thật.

### 2i. Danh sách Tải xuống: theo thứ tự chương, mục đã xong rời khỏi hàng đợi — 2026-10-07

User: "List tải xuống xếp theo thứ tự chapter từ bé đến lớn, chapter nào hoàn thành tự động xoá khỏi list tải xuống".

**(1) Thứ tự.** `list_tasks` xếp `priority DESC, created_at ASC, id ASC` — tức đúng cái lỗi mà `claim_next` đã sửa hôm 01/10 (`queue_batch` đóng một `created_at` cho cả batch, nên tiebreak rơi xuống UUID). Nay `TaskRow`/`TASK_SELECT`/`DownloadTaskDto` mang thêm `chapter_number` (`c.number`, `#[sqlx(default)]` vì không phải query nào cũng join `chapters`), và cả hai nhánh của `list_tasks` dùng đúng biểu thức của `claim_next`: `priority DESC, c.number IS NULL, c.number, c.created_at, t.id`. Frontend sort lại trong `visible` bằng `byQueueOrder` cùng khóa — để một event tới sau (`unshift`) không chen lệch thứ tự.

**(2) "Tự xoá" — hiểu là rời khỏi danh sách, KHÔNG phải xoá row.** Row `completed` là bằng chứng offline (chính nó nói chương này có file trong Thư viện), và dialog "Dọn lịch sử" tồn tại để người dùng quyết định xoá. Nên cách làm là mặc định của trang đổi từ "Tất cả" sang chip MỚI **"Hàng đợi"** = mọi trạng thái trừ `completed`: chương vừa xong biến mất ngay khi event về, không cần tải lại, còn lịch sử vẫn xem được bằng chip "Hoàn thành". Không có thao tác xóa dữ liệu nào trong bản này.
Header đếm theo `visible.length/total` để không nói "47 tác vụ" khi đang xem 5.

Test: `list_tasks_orders_by_chapter_number` (seed 3-1-2-khôngsố, chốt 1-2-3-∅) và `tests/downloadsQueueOrder.test.tsx` (3 cái: thứ tự tăng dần dù server trả lộn xộn; completed bị ẩn nhưng chip Hoàn thành vẫn thấy; completed qua event thì rời danh sách ngay).

### 2j. Chương `tap-<số>-<tên>` không có số → Thư viện lộn thứ tự — 2026-10-07

User gửi ảnh trang truyện trong Thư viện: "Đã có file (7)" xếp Tập 1, 6, 2, 7, 4, 3, 5. `list_chapters` vẫn xếp `number IS NULL, number, created_at, id` đúng — vấn đề là **cả 12 chương của truyện đó có `number = NULL`**, nên thứ tự thực tế là thứ tự ghi vào DB.

Nguyên nhân: `chapter_number_from_slug` chỉ nhìn **cụm số CUỐI slug** (đúng cho `…-chap-2-1` = 2.1). Slug của TTT là `tap-1-dai-tuong-ra-doi` — kết thúc bằng một từ, không bao giờ khớp luật đó. DB user: 12/168 chương NULL, tất cả thuộc đúng một truyện `mankichi-dai-tuong-nhoc-con-full`.

Sửa hai phần:
1. **Luật tiền tố** trong cùng hàm: `^(tap|chap|chapter|episode|program|part)[-_ .]*(\d+(?:[.,]\d+)?)(?:$|[-_ .])`. Chỉ chạy khi **không có** cụm số ở cuối, nên họ `chap-2-1` giữ nguyên kết quả; và khi slug kết thúc bằng số nhưng mơ hồ (≥2 nhóm) thì vẫn trả `None` — không đoán chồng lên một slug nguồn đã viết không nhất quán.
2. **Chữa row cũ**: `library_service::backfill_chapter_numbers(pool)` gọi một lần trong `setup()` (sau `reconcile_startup`). `UPDATE … WHERE id = ?1 AND number IS NULL` — chỉ điền chỗ trống, không bao giờ ghi đè số mà một lần refresh đã lưu, và bỏ qua slug không đọc được số. Chạy mỗi lần khởi động nên idempotent; log số row đã chữa.

Test: `chapter_number_reads_a_leading_number_when_the_slug_starts_with_it` (kèm 3 case "không có số thì đừng bịa") và `backfill_fills_only_unnumbered_chapters_and_orders_the_list` (seed lộn xộn 3-1-2 + 1 row đã có số + 1 row không đọc được → đúng 3 row được điền, chạy lần hai = 0, danh sách ra 1-2-3-9-∅).

### 2k. "Lỗi nội bộ không xác định" lần hai — mọi bước io phải tự gọi tên — 2026-10-22

User báo lại đúng dòng "Không đổi được nơi lưu — Lỗi nội bộ không xác định." sau khi lỗi junction treo đã sửa. Kiểm tra hiện trường: junction `appdata\library → H:\My Drive\library` **vẫn lành** (6 thư mục truyện đọc xuyên qua nó), settings vẫn `H:\My Drive` — tức lần này thao tác thất bại TRƯỚC khi kịp đổi gì (rollback giữ nguyên hiện trường, đúng thiết kế).

Bằng chứng thủ phạm: `Get-PSDrive` trên máy user trả **`H: FREE_GB = 0`** (Google Drive hết hạn mức/đồng bộ). Mọi `std::fs` lỗi khi tạo thư mục/sao chép vì thế là chuyện bình thường ở cấu hình đó — nhưng code chỉ `?` thẳng `io::Error` → wire `INTERNAL_ERROR` → người dùng không biết sửa gì.

Sửa trong `services/storage_root.rs`: `io_reason()` dịch các mã Windows hay gặp (2/3 không tìm thấy, 5 từ chối quyền, 32/33 tệp bị giữ, 36/206 quá dài, 112 **hết chỗ trống**, 123/267 tên không hợp lệ) sang tiếng Việt, còn `stage_io()` gắn thêm bước gây lỗi. Áp dụng ở: tạo thư mục đích, duyệt danh sách tệp nguồn, tạo thư mục cha từng tệp, `copy`, `stream_equal`/`read_partial` (giờ nhận `stage`), `rename` publishing, `remove_junction` fallback, dời `library` cũ sang chỗ tạm, và staging của `reset_to_default`. Không còn đường nào trong `relocate`/`reset_to_default` để `AppError::Io` lọt lên UI.

Test: `an_unusable_destination_names_its_stage` (đích nằm dưới một TỆP → message phải bắt đầu "Không tạo được \`" và không chứa chữ "nội bộ") và `io_reason_translates_the_codes_users_actually_hit`.

**Đính chính từ chính user:** thao tác thất bại là khi **chuyển sang ổ J** (một mount Google Drive khác, 25 GB trống) — không phải H:. Chi tiết này đổi cách sửa: ngoài việc gọi tên bước, `relocate` còn **chặn từ đầu khi nơi đích không đủ chỗ** (`downloaded_bytes` = `SUM(local_size_bytes)` của chương đã tải, so với `volume_space(new_base)`, làm tròn theo hướng thận trọng cả hai phía) — vì sao chép dở rồi mới fail vừa mất hàng chục phút vừa để lại một nửa bản sao. Thư viện chỉ bị bỏ bản cũ SAU khi bản mới được xác minh từng byte, nên cần đủ chỗ cho cả hai. Test: `relocate_refuses_a_destination_without_room` (khai báo 4 TB đã tải → từ chối, và `store/library` KHÔNG được tạo ra).

**Hệ luận cần nhớ khi ai đó hỏi "vì sao chuyển nơi lưu thất bại":** nhìn `FREE_GB` của ổ nguồn và ổ đích trước khi nghi ngờ code. Cả H: (0 GB) và J: (25 GB) ở máy user đều là mount của Google Drive — để một thư viện nhiều GB trên đó vốn đã mong manh, và junction của app giả định một ổ đĩa cục bộ.

## 3. Bất biến kiến trúc (không được phá)

1. Mọi `invoke()` app-command đi qua `src/shared/api/desktop.ts`; window API đi qua `src/services/windowService.ts`.
2. capability chỉ window `main`: `core:default` + 5 window permission tường minh; **không** fs/shell/http; CSP không wildcard — muốn mở quyền phải có brief phase cho phép.
3. SQLite chỉ từ Rust; DB tại `app.path().app_data_dir()` (identifier `com.manga1989.downloader`); migration append-only, không sửa `0001_init.sql` đã release. Ngoại lệ có kiểm soát duy nhất: SQL của migration `0004` được THAY tại runtime bằng bản repaired (`src/db/migrations_repaired/`, giữ nguyên checksum gốc để DB đã apply không lệch validate) — chi tiết + test thuận/đảo trong `src/db/mod.rs`.
4. Lỗi IPC có cấu trúc `{code, message, retryable}` đã sanitize — không path/SQL/stack trace.
5. Health phải thật: `app_health_check` chạy `SELECT 1` thật; UI không hardcode "ready".
6. Không `Math.random()`/clock trong fixture; test phải `resetMockState()` được.
7. `AGENTS.md` là file của user — không sửa/xóa.
8. (Phase 05) Mọi HTTP tới provider đi qua `GuardedHttp` với exact-host allowlist của adapter sở hữu; renderer chỉ gửi query/ID/URL-để-đối-khớp, không bao giờ host/headers/endpoint tùy ý. Save metadata luôn re-fetch phía Rust trong 1 transaction; không xóa chapter theo response thiếu.

## 4. Việc còn tồn (chưa có phase nào giao)

- [ ] Chạy thử **bản cài đặt** (MSI/NSIS trong `src-tauri/target/release/bundle/`, gitignore) + xác nhận titlebar drag/resize/snap, scaling 100–200%, screenshot các màn hình tại 1440×900 và 1024×700 → đính vào docs phase sau.
- [x] ~~`npm run tauri build` lại từ code mới~~ — **đã chạy Phase 09 (EXIT=0, MSI+NSIS 2026-09-23)**; còn lại: **cài + smoke installer thật** theo checklist `docs/phase-09/WINDOWS_SMOKE.md` (13 mục manual, gồm live download R-1).
- [ ] Phase 05 live pending: bấm thử Discover live + Import URL với `truyentuoitho.com` trên máy user (WAF site reset request dồn — client đã pace ≥3s); MangaDex vẫn BLOCKED ở mạng dev.
- [ ] **Phase 06 live pending (T06-24b):** tải thật 1 chapter → CBZ trên Windows với nguồn Madara cùng host allowlist; nếu host ảnh khác host site → engine chặn, cần source-policy review.
- [ ] Xác nhận **đóng app bằng nút X** dừng worker sạch (shutdown đã nối `RunEvent::ExitRequested`; cần test trên bản `tauri build`) — phục vụ packaging gate.
- [ ] **Phase 10 còn mở (blocker hợp lệ, không phải defect):** (a) chạy `docs/phase-10/WINDOWS_INSTALL_UPGRADE_TEST.md` 15 mục trên VM/profile sạch; (b) ~~live download journey R-1~~ ĐÓNG 2026-09-23 ở mức headless (probe `#[ignore] live_r1`: Ninja Loạn Thị ch.1 từ truyentuoitho.com, 188/188 trang → CBZ 49.578.260 B (≈47,3 MiB) verify, attempts=0; phát sinh feature image-host allowlist + AJAX chapter-list); còn xác nhận thao tác thực tế trong app UI; (c) Codex Reviewer ACCEPT RC + owner approval + Release Engineer publish + verify remote hash (T10-39/40); (d) nếu phân phối công khai: quyết định code-signing cert + license audit + kế hoạch updater riêng.
- [ ] ~~Icon app thật~~ — đã làm ở Phase 10 (`scripts/make-icon.mjs` + `npx tauri icon`, full set trong `src-tauri/icons/`).
- [ ] Cân nhắc self-host font Inter (CSP `'self'` cấm CDN).
- [ ] `AppError::Internal` (Rust) giờ đã dùng ở các đường panic-safe của download engine.
- [ ] **Phase 07 manual (T07-24..32):** verify trên Windows đóng gói — fullscreen thật, click-X khi đang đọc, reader chapter 300 trang thật, library ~1k bản ghi, ngắt mạng khi đọc offline. `docs/phase-07/test-results.md` đã đánh NOT RUN thẳng thắn.
- [ ] Deflate CBZ (mở archive từ nguồn ngoài) — hiện trả `CBZ_INVALID` có kiểm soát theo contract Stored-only; chỉ làm khi có nhu cầu thật.
- [ ] **Phase 08 manual (checklist P09):** bấm thử toast thật (cần download live), tray Show/Quit, close-to-tray X khi đang tải, palette Ctrl+K trên app thật, High-DPI 125/150% — `docs/phase-08/test-results.md` đánh NOT RUN từng mục.
- [ ] Explicitly deferred từ P08 (có lý do trong `docs/phase-08/settings-and-storage-policy.md`): clear download history, đổi library root, "open folder" (cần capability opener mới — phải qua brief phase cho phép).

## 5. Bắt đầu Phase 04+ thế nào

1. User cung cấp brief phase mới (các brief hiện có trong `docs/briefs/`).
2. Quy trình đã thống nhất: **inspect repo → implement theo task trong brief → tự review đối chiếu brief (BLOCKING/ADVISORY, sửa ngay BLOCKING) → chạy gates → commit local → report**. Bước reviewer ngoài đã được user bỏ (2026-09-22).
3. Lệnh chuẩn: xem `README.md` §Quality Gates. Không push/merge/deploy khi user chưa yêu cầu rõ.
4. Kết thúc mỗi phase: cập nhật `docs/phase-XX/` + bảng §1 của file này.
5. **Contract Phase 06 (đã user chốt 2026-09-22):** tải thuần ảnh chương từ nguồn đã đăng ký → nén thành file `.cbz`; pipeline: `get_page_manifest` nội bộ theo adapter, checkpoint `download_pages` (schema có từ 0002), hàng đợi qua `download_tasks`; không đổi CSP/capability.

## 6. Cấu trúc docs

```
docs/
├── briefs/          # brief gốc Phase 01–03 (nguồn yêu cầu)
├── plans/           # implementation plan đã duyệt Phase 01
├── phase-01|02|03|04|05|06|07|08|09|10/  # implementation/notes + self-review + test-results từng phase; phase-09 có QA_MATRIX + evidence/; phase-10 có RUNBOOK + MANIFEST + TEST_MATRIX
├── IPC_CONTRACT.md   # hợp đồng command/DTO/error (Phase 04 + 05)
└── HANDOFF.md       # file này
```
