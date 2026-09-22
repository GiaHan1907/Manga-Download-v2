# PHASE_08_HANDOFF — Manga Downloader 1989

**Ngày:** 2026-09-23 · **Commit baseline (Phase 07 ACCEPT + review 3 vòng):** `0f81dac`
**Trạng thái:** hoàn tất implementation + gates; chờ Codex Reviewer theo quy trình `AGENTS.md`.

## 1. Summary

**Đã làm:** audit toàn bộ surface (integration-map); invalidation bus + AppEventBridge thay cho state lib mới; route guard 404 + cách ly sandbox demo về dev-only; Home 100% số liệu thật (`library_stats` + recent tasks); Favorites + Downloads pagination; Command Palette Ctrl+K (local-only, keyboard, XSS-inert); Settings production (2 key mới validate Rust + RuntimeFlags sync); storage accounting + clear-regenerable-cache an toàn (kèm startup pass); Windows notification theo chapter-terminal + tray Show/Hide·Open Downloads·Quit + close-to-tray policy.

**Có điều kiện / chưa làm (khai thẳng):** T08-16 clear-history (từ chối làm nút nửa vời), T08-20 đổi library root, open-folder (cần capability mới), free-space số tuyệt đối (không thêm dep), live journey T08-36 BLOCKED như T06-24b, `tauri build` NOT RUN từ P04, toàn bộ thao tác thủ công Windows (toast thật, tray click, X khi đang tải, High-DPI) = NOT RUN — danh sách cho Phase 09 trong `test-results.md`.

## 2. Bằng nền Phase 07

`docs/phase-07/self-review.md` + commit `0f81dac` (3 vòng review, gates xanh). Phase 08 build trực tiếp trên code đó, không rewrite.

## 3. Task traceability

| Task | Kết quả | Ghi chú scope |
|---|---|---|
| P08-01 | xong | `integration-map.md` — mọi route/command/schema đối chiếu code thật |
| P08-02 | xong (mức automated) | Deviation CÓ CHỦ ĐÍCH: không thêm TanStack/Zustand (plan §2.7 + AGENTS.md "no unnecessary deps; existing structure wins") → `useInvalidation` + `AppEventBridge` thay `qk` keys; semantic tương đương: event chỉ invalidate, snapshot vẫn từ Rust |
| P08-03 | xong | reuse `library_list` (parameterized, bounded) — không FTS, không remote tự động |
| P08-04 | xong trừ clear-history | favorites/continue/history dùng đúng bảng cũ, không duplicate schema |
| P08-05 | xong | `notificationsEnabled`, `closeToTray` thật-sự-consumed; bỏ PDF khỏi UI; dirty-state save |
| P08-06 | xong trừ đổi-root/open-folder | cleanup + status mới; startup auto-cleanup sau recovery |
| P08-07 | xong (code+smoke) | notification Rust-side → **không đổi capabilities**; tray dùng feature `tray-icon` |
| P08-08 | xong | Home/Topbar/404/demo-gating copy sạch mock leakage trong production build |
| P08-09 | file này + docs | — |

## 4. Files changed

**Rust mới/sửa chính:** `domain/settings.rs` (+2 keys), `domain/mod.rs` (3 DTO), `app_state.rs` (`RuntimeFlags`), `commands/settings.rs` (flag sync), `commands/downloads.rs` (notify sink), `commands/storage.rs` (MỚI), `commands/library.rs` (`library_stats`), `services/storage_service.rs` (status/cleanup + test), `services/library_service.rs` (stats, progress fallback), `downloader/storage.rs` (`valid_task_id_shape` pub(crate)), `downloader/recovery.rs` (dùng helper), `lib.rs` (plugin notification, tray, on_window_event, startup cleanup, 3 commands), `Cargo.toml` (`tray-icon`, `tauri-plugin-notification`).

**Frontend mới/sửa chính:** `shared/hooks/useInvalidation.ts` (MỚI), `app/downloadInvalidation.ts` (MỚI), `app/AppEventBridge.tsx` (MỚI), `features/global-search/CommandPalette.tsx` (MỚI), `HomePage.tsx` (rewrite real), `SettingsPage.tsx` (rewrite + Windows + storage panel), `FavoritesPage.tsx` (pagination), `DownloadsPage.tsx`+`useDownloadQueue.ts` (paged snapshot + total), `DiscoverPage.tsx`+`LiveSearchPanel.tsx` (?q prefill, dev-gate, sources domain), `RemoteDetailPage.tsx`, `LibraryPage.tsx`, `LocalMangaPage.tsx`, `useReaderProgress.ts` (bump cross-screen), `LocalContinueReading.tsx`, `AppShell.tsx`, `Topbar.tsx` (palette trigger), `router.tsx` (dev-only demo routes), `shared/api/desktop.ts` (3 command wrappers + `subscribeTrayOpenDownloads`), `types/backend.ts`, `services/settings.*` (+2 keys), `styles/globals.css` (palette/storage).

**Tests:** MỚI `tests/phase08.test.tsx` (5); cập nhật có chủ đích `app.test.tsx`, `phase03`, `phase04`, `phase06` theo UI Phase 08; Rust +3 `t08_*`.

## 5. Migrations & settings

KHÔNG migration mới (schema 5 giữ nguyên; settings là blob JSON). Key mới + range + effective timing: `settings-and-storage-policy.md` §1. Không duplicate favorites/progress/history.

## 6. IPC & events

- Commands mới: `library_stats`, `storage_status`, `clear_regenerable_cache` (contract: `docs/IPC_CONTRACT.md` §Phase 08).
- Event mới: `tray-open-downloads` (Rust→JS, payload rỗng — tín hiệu điều hướng).
- `download-task-updated`: không đổi payload; consumer app-level duy nhất (bridge) + per-screen queue merge; mọi handler guard `disposed`, unlisten đầy đủ (T08-35).
- Invalidation: 7 domain (home/library/favorites/downloads/progress/settings/sources) — UI transient, persisted truth luôn refetch từ Rust.

## 7. Storage & Windows behavior

Toàn bộ trong `settings-and-storage-policy.md` §2–§3: root không đổi; cleanup chỉ staging terminal/orphan + covers; idempotent; active-set chặn mọi thứ đang chạy; startup pass sau recovery; tray/quit/close policy; không thêm capability nào.

## 8. Test evidence

`test-results.md`: gates (vitest 81, cargo 104, clippy/fmt/build/typecheck, `tauri dev` smoke với plugin+tray) + ma trận T08-01..36 với PASS/PASS(mức wiring)/NOT IMPLEMENTED(có chủ đích)/NOT RUN/BLOCKED.

## 9. Known issues & độ ưu tiên cho Phase 09

1. **BLOCKED live:** T08-36/T06-24b — một người phải chạy thật download + toast + tray trên máy (checklist có trong policy §2).
2. `tauri build` chưa chạy từ Phase 04 → bắt buộc trước release (verify tray icon + notification trong bundle thật; Windows toast cần app user-model ID — có thể phải thêm `productName`/AUMID handling).
3. Downloads page: filter áp trên trang 100 phần tử đang xem (multi-status server filter chưa có) — đủ dùng, nhưng là điểm QA Phase 09.
4. Reader `reader_get_page_bytes` mở file mỗi trang — re-parse CBZ per request (chấp nhận desktop-local; profile trước khi optimize).
5. cbz STORE-only (deflate defer); scroll-fraction restore vertical chưa đo trên layout thật.
6. Palette dùng `library_list` — với library rất lớn có thể muốn endpoint search riêng nhẹ hơn (đo trước, đừng đoán).

## 10. Repro baseline cho Phase 09

```bash
npm ci
cd src-tauri && cargo build   # cần tauri-plugin-notification (crates.io)
npm run typecheck && npm run lint && npm test && npm run build
cd src-tauri && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test
npm run tauri dev   # smoke: tray icon + menu, Home số thật, Ctrl+K palette
```

Không secret/token trong code, log hay docs. Không push/deploy — theo quy trình user-triggered.
