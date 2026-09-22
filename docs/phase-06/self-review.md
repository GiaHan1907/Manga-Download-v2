# Phase 06 — Self-review (đối chiếu brief, phát hiện & sửa trong phiên)

Review theo §11 Definition of Done + §2 non-negotiables. **BLOCKING** = phải sửa trước khi bàn giao (đã sửa ở dưới). **ADVISORY** = ghi nhận, không chặn.

## Vòng 1 — phát hiện

| # | Loại | Finding | Xử lý |
|---|---|---|---|
| F1 | BLOCKING | `verify_cbz` dùng `|| crc_hex_matches(...)` thiếu `!` → archive hợp lệ bị báo hỏng & archive giả không bị bắt | Sửa `!crc_hex_matches`; test `t06_17` + `tampered_archive` xanh |
| F2 | BLOCKING | `sanitize_label` không biến `/ : .` thành dash, so reserved-name sai case → `..`/`nul.txt`/`CON` thoát kiểm tra Windows | Rewrite mapping separator + reserved stem ignore-case; test T06-19 |
| F3 | BLOCKING | `image_validate::validate` trả TRUNCATED trước khi phát hiện HTML nhỏ → phân loại lỗi sai | Đổi thứ tự: NOT_IMAGE trước TRUNCATED |
| F4 | BLOCKING | Manifest co lại giữa attempt để lại `download_pages` pending vượt `page_urls.len()` → worker `page_urls[index]` panic | `ensure_page_rows` prune pending surplus; worker filter `index < len`; test `shrinking_manifest_prunes_surplus_pending_pages` |
| F5 | BLOCKING | Worker panic làm leak slot trong `active` map (mã `post-await` không chạy) → nghẽn dispatcher | RAII `SlotGuard` trong `spawn_worker` |
| F6 | BLOCKING | `queue_chapter_downloads` giữ `registry` mutex (std) **qua `await`** của DB query → rủi ro deadlock/serialize, vi phạm §2.7 | Tách: resolve chapter (không lock) → rồi capability check trong scope lock ngắn, không await |
| F7 | ADVISORY | `PageManifest.source_chapter_id`, `ExportPage.index`, `StagedPage.path`, `bump_attempt`, `NullSink` dead-code (clippy -D) | Dùng `source_chapter_id` làm integrity guard trong worker; `index` để sort xuất; bỏ `path`/`bump_attempt`/`NullSink`; `CollectingSink` thành `#[cfg(test)]` |
| F8 | ADVISORY | `into_string()` deprecated (Url) | → `to_string()` |
| F9 | ADVISORY | `fail_or_requeue` dùng `let-else` bị clippy gợi ý; closures redundant | Dọn theo clippy; `cargo fmt` |
| F10 | BLOCKING (tài liệu, vòng 3) | `self-review.md`/`handoff.md`/`HANDOFF.md` ghi `shutdown()` "chưa gắn vào ExitRequested" — **sai**: code `lib.rs` đã wire `.build().run()` + `RunEvent::ExitRequested → block_on(downloads.shutdown())` ngay từ P06-06 | Sửa tài liệu khớp code (đã làm vòng 3); code không đổi. Bài học: tài liệu phải sinh SAU khi đối chiếu code, không từ trí nhớ |
| F11 | ADVISORY | Hành vi đóng-app tương tác (click X) chưa chạy thủ công | `shutdown()` được gọi khi exit; restart-recovery vẫn là backstop cho hard crash (T06-21 PASS fixture). Xác nhận click-X trên bản `tauri build` thuộc packaging gate còn mở |

## Vòng 2 — kiểm chứng lại sau sửa

- `cargo clippy --all-targets -- -D warnings`: **0 warning** → mọi dead-code/BLOCKING F7–F9 đã xử lý thật (không `#[allow]` trừ khi có lý do).
- `cargo test`: **84 pass** (thêm F4 prune test). `cargo fmt --check`: pass.
- vitest **65 pass**, typecheck/build/lint pass.
- `tauri dev` smoke: DB app-data thật upgrade 3→4 sạch, không panic, dispatcher chạy.

## Đối chiếu §11 exit gate (trạng thái trung thực)

- [x] Phase 05 gate theo HANDOFF (round1+2 PASS; reviewer ngoài đã user gỡ). Provider permission: **Madara** có sở cứ (user-declared, nút "Tải trọn bộ" của site, chỉ dùng endpoint public); **MangaDex** → capability `pages` KHÔNG declare (at-home host-dộng xung đột exact-host). Documented, không fake.
- [x] Page manifest backend-only, scoped transport; không có IPC URL/header/path tùy ý (§2.1, invariant §3.1/3.8 giữ nguyên).
- [x] State machine + bounded scheduler + retry + pause/resume/cancel + persistence, transition hợp lệ enforced server-side (§2.7).
- [x] Per-page checkpoint + reconcile 2 crash boundary (page-rename, CBZ-publish) — T06-15/16.
- [x] Folder + CBZ output đủ thứ tự, atomic, không đè file lạ (§2.5).
- [x] Chỉ output verified mới `completed`/offline; catalog/fav/progress giữ nguyên (T06-01).
- [x] UI typed Rust API/events, không Demo leakage ở live view, không fake speed/progress.
- [~] T06-01..24: **PASS fixture** + 24b **LIVE = BLOCKED/PENDING-MANUAL**; `tauri build` = NOT RUN. → **KHÔNG** tự tuyên bố Phase 06 feature-complete cho live download (đúng §11 gate discipline).
- [ ] Codex Reviewer ACCEPT: quy trình hiện hành là user bỏ reviewer ngoài (HANDOFF §5.2) → bàn giao + chờ user. Không push/deploy.

## Kết luận

Không còn BLOCKING mở trong phạm vi fixture/engine; `shutdown()` đã được nối `ExitRequested` (chỉ tài liệu từng ghi sai, đã sửa ở F10). Hai mục **trung thực còn treo**: (1) live-source end-to-end CBZ, (2) `tauri build` packaging (kèm xác nhận thủ công đóng-app click-X) — đều ghi ở `handoff.md`, không che.

---

## Review 3 vòng sau commit `63344cf` (chức năng Codex Reviewer — chỉ đọc, không sửa code; Fixer sửa BLOCKING rồi re-review)

### Vòng 1 — tính đúng đắn dữ liệu & lifecycle
Đọc lại `download_repo` + `worker` với mắt nhìn mới (không tin ghi chú cũ).
- **BLOCKING R1-1**: manifest co lại giữa attempt để lại `download_pages` `stored` thừa ⇒ `report.valid != total_pages` và `exports.len() != total_pages` fail **vĩnh viễn** mọi retry (task kẹt `failed`). → FIX: `ensure_page_rows` xóa MỌI row thừa `page_index >= total` (kể cả stored — identity theo vị trí); worker thêm guard `page_index >= total` → fail-safe; test `shrinking_manifest_prunes_surplus_pages_of_any_state`.
- **ADVISORY R1-2**: cancel tới trong lúc `exporting` chỉ được thấy ở boundary. → FIX ở vòng 3.
- **REJECT** (còn BLOCKING) → Fixer sửa → re-review.

### Vòng 2 — hợp đồng UI ↔ backend
So `allowedDownloadActions` (TypeScript) với các `transition` được Rust chấp nhận.
- **BLOCKING R2-1**: UI cho `pausing` → "resume", nhưng backend `resume_download` chỉ nhận `paused` ⇒ người dùng bấm `resume` lúc `pausing` gặp lỗi. → FIX: `pausing` chỉ còn `["cancel"]` (chờ ack), comment giải thích; `paused` giữ resume/cancel.
- **ADVISORY R2-2**: hàng đợi mặc định đọc Settings; `output_format=pdf` được map về `cbz` (pdf từ chối ở tầng task) — tài liệu `handoff.md`/design đã nêu; giữ.
- **REJECT** → Fixer sửa → re-review.

### Vòng 3 — concurrency / race / tài liệu
- **BLOCKING R3-1**: cancel giữa chunk làm `fetch_and_store_page` trả `AppError::Internal` → `fail_or_requeue` gắn nhãn `failed` (sai, phải là `cancelled`). → FIX: `join_next` vòng lặp kiểm `cancel` TRƯỚC khi phân loại lỗi ⇒ luôn ra `cancelled`; regression test `cancel_observed_midflight_lands_cancelled_not_failed`.
- **BLOCKING R3-2**: `spawn_worker` early-return khi `active >= MAX_TRACKED_TASKS` để lại task đã claim ở `resolving` **không có worker** (zombie vi phạm §2 "never show downloading without a live worker"). → FIX: dồn giới hạn `MAX_TRACKED_TASKS` vào `capacity` ở `dispatch_available` (trước `claim_next`), `spawn_worker` không còn nhánh từ chối ⇒ task đã claim luôn có worker.
- **BLOCKING R3-3 (tài liệu-vs-code)**: `self-review.md`/`handoff.md`/`HANDOFF.md` ghi `shutdown()` "chưa nối ExitRequested" — SAI, `lib.rs` đã nối từ P06-06. → FIX tài liệu khớp code (F10/F11 viết lại); code không đổi.
- Kiểm lại an toàn còn nguyên: cancel không xóa file user đã publish; ZIP basename-only; exact-host; không IPC URL/path.

### Verdict cuối
Vòng 3 re-review **không còn BLOCKING** → **ACCEPT (fixture/engine)**. Không còn mục nào tự gán "PASS" quá xa hơn gate thật: live-source CBZ vẫn **PENDING-MANUAL/BLOCKED**, `tauri build` vẫn **NOT RUN**.

### Gates sau mỗi vòng (Fixer bắt buộc chạy)
| Vòng | cargo test | clippy -D | fmt | vitest | typecheck | lint | build |
|---|---|---|---|---|---|---|---|
| 1 (fix R1) | 84 ✓ | ✓ | ✓ | 65 ✓ | ✓ | ✓ | ✓ |
| 2 (fix R2) | 84 ✓ | ✓ | ✓ | 65 ✓ | ✓ | ✓ | ✓ |
| 3 (fix R3 + test mới) | 85 ✓ | ✓ | ✓ | 65 ✓ | ✓ | ✓ | ✓ |
