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
| F10 | (khai báo) | App-shutdown `Coordinator::shutdown` chưa được gọi → dead-code | **Không** wire im lặng: gọi qua `RunEvent` cần cân nhắc runtime; giữ `#[allow]`? → KHÔNG, đã giữ là API public `pub async fn`; clippy hết báo khi có Exit handler. DỜI sang advisory F11. |
| F11 | ADVISORY | `shutdown()` chưa có caller trong `lib.rs` (ExitRequested handler bị skip để tránh đụng main-loop khi chưa verify runtime context) | Ghi chú handoff: nối `RunEvent::ExitRequested → block_on(shutdown)` khi làm packaging gate; recovery đã đảm bảo nhất quán khi app chết đột ngột, nên KHÔNG phải BLOCKING |

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

Không còn BLOCKING mở trong phạm vi fixture/engine. Hai mục **trung thực còn treo**: (1) live-source end-to-end CBZ, (2) `tauri build` + ExitRequested→shutdown wiring — đều ghi ở `handoff.md`, không che.
