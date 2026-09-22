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
| 07 — Manga Library & Offline Reader | Migration `0005` (forward-only ALTER: `mangas.last_read_at`, `chapters.local_size_bytes`, `reading_progress.scroll_offset/page_total` + index); library backend aggregate 1-SQL (counts/size/progress/has-missing, 4 sort × 4 filter × search escape), delete disk-first path-safe, reconcile non-destructive; reader backend: manifest thống nhất CBZ (tail-window EOCD + central-dir segment, STORE-only, sniff ≤12 trang)/image-folder, page bytes per-page qua `tauri::ipc::Response` (không base64-cả-file, không lộ path); UI Library chi tiết (grid/list, trạng thái thật, xóa confirm); Offline Reader `/read/:chapterId`: 3 mode + LTR/RTL + zoom + fullscreen(thật→fallback CSS) + keyboard, blob LRU revoke, prefetch; progress debounce 800ms + flush ranh giới chapter/unmount, "Đọc tiếp" THẬT ở Home/detail; 7 IPC commands (`docs/IPC_CONTRACT.md` §Phase 07) + 8 error codes `LibraryError` | `docs/phase-07/self-review.md` (3 BLOCKING đã sửa: cbz whole-file read, sniff-all-pages, progress-write-mỗi-trang) + `test-results.md` (cargo 100/100 = +9 `t07_*` +6 unit reader; vitest 75/75 = +7 readerLogic +3 phase07 UI; `tauri dev` smoke nâng DB thật schema 4→5) | `docs/phase-07/{reconnaissance,library-reader-contract,integration-notes,handoff}.md` | ACCEPT; T07-24..32 Windows-manual NOT RUN; deflate CBZ defer (xem handoff) |

**Gate hiện hành (2026-09-23, sau Phase 07):** lint · typecheck · vitest 75/75 · vite build · cargo fmt/check/clippy(`-D warnings`)/test 100/100 · `tauri dev` smoke + DB thật lên **schema 5**. `tauri build` packaging: **NOT RUN** từ Phase 04 tới giờ — gate bắt buộc trước release.

## 2. Trạng thái "thật" từng phần (sau Phase 07)

- **Thật (Rust + SQLite + đĩa):** Settings, Library/Favorites local, discovery Phase 05 (`localServices.source`), toàn bộ download engine Phase 06 (`localServices.downloads`), **Library chi tiết + Offline Reader Phase 07** (`localServices.libraryV2` / `.reader`: aggregate/reconcile/delete thật, đọc CBZ-folder thật, progress thật, "Đọc tiếp" thật ở Home).
- **Vẫn là demo:** Manga Detail tab demo sandbox + route `/reader/:mangaId/:chapterId` cũ (mock), favorites ★ trong Manga Detail demo, panel "Continue Reading" demo trong Home (strip THẬT nằm riêng phía trên, có nhãn). Nguồn dữ liệu demo: `src/services/mock/`.
- **Live download:** engine sẵn sàng nhưng **T06-24b live = PENDING-MANUAL/BLOCKED** (mạng dev block MangaDex; Madara cần user thử + host ảnh phải thuộc allowlist exact-host). Không tuyên bố live PASS. Pipeline offlineCBZ→Reader đã khép kín ở mức FIXTURE (exporter Phase 06 sinh file thật, reader Phase 07 đọc lại trong test).

## 3. Bất biến kiến trúc (không được phá)

1. Mọi `invoke()` app-command đi qua `src/shared/api/desktop.ts`; window API đi qua `src/services/windowService.ts`.
2. capability chỉ window `main`: `core:default` + 5 window permission tường minh; **không** fs/shell/http; CSP không wildcard — muốn mở quyền phải có brief phase cho phép.
3. SQLite chỉ từ Rust; DB tại `app.path().app_data_dir()` (identifier `com.manga1989.downloader`); migration append-only, không sửa `0001_init.sql` đã release.
4. Lỗi IPC có cấu trúc `{code, message, retryable}` đã sanitize — không path/SQL/stack trace.
5. Health phải thật: `app_health_check` chạy `SELECT 1` thật; UI không hardcode "ready".
6. Không `Math.random()`/clock trong fixture; test phải `resetMockState()` được.
7. `AGENTS.md` là file của user — không sửa/xóa.
8. (Phase 05) Mọi HTTP tới provider đi qua `GuardedHttp` với exact-host allowlist của adapter sở hữu; renderer chỉ gửi query/ID/URL-để-đối-khớp, không bao giờ host/headers/endpoint tùy ý. Save metadata luôn re-fetch phía Rust trong 1 transaction; không xóa chapter theo response thiếu.

## 4. Việc còn tồn (chưa có phase nào giao)

- [ ] Chạy thử **bản cài đặt** (MSI/NSIS trong `src-tauri/target/release/bundle/`, gitignore) + xác nhận titlebar drag/resize/snap, scaling 100–200%, screenshot các màn hình tại 1440×900 và 1024×700 → đính vào docs phase sau.
- [ ] `npm run tauri build` lại từ code Phase 06 (gate trước release; chưa chạy từ Phase 04).
- [ ] Phase 05 live pending: bấm thử Discover live + Import URL với `truyentuoitho.com` trên máy user (WAF site reset request dồn — client đã pace ≥3s); MangaDex vẫn BLOCKED ở mạng dev.
- [ ] **Phase 06 live pending (T06-24b):** tải thật 1 chapter → CBZ trên Windows với nguồn Madara cùng host allowlist; nếu host ảnh khác host site → engine chặn, cần source-policy review.
- [ ] Xác nhận **đóng app bằng nút X** dừng worker sạch (shutdown đã nối `RunEvent::ExitRequested`; cần test trên bản `tauri build`) — phục vụ packaging gate.
- [ ] Icon app thật (thay placeholder 16×16): `npx tauri icon <png>` trước khi release.
- [ ] Cân nhắc self-host font Inter (CSP `'self'` cấm CDN).
- [ ] `AppError::Internal` (Rust) giờ đã dùng ở các đường panic-safe của download engine.
- [ ] **Phase 07 manual (T07-24..32):** verify trên Windows đóng gói — fullscreen thật, click-X khi đang đọc, reader chapter 300 trang thật, library ~1k bản ghi, ngắt mạng khi đọc offline. `docs/phase-07/test-results.md` đã đánh NOT RUN thẳng thắn.
- [ ] Deflate CBZ (mở archive từ nguồn ngoài) — hiện trả `CBZ_INVALID` có kiểm soát theo contract Stored-only; chỉ làm khi có nhu cầu thật.

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
├── phase-01|02|03|04|05|06|07/  # implementation/notes + self-review + test-results từng phase
├── IPC_CONTRACT.md   # hợp đồng command/DTO/error (Phase 04 + 05)
└── HANDOFF.md       # file này
```
