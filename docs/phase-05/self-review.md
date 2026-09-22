# Phase 05 Self-review — các vòng

Quy trình đã đổi theo chỉ đạo user (2026-09-22): không có reviewer ngoài; implementer tự review đối chiếu brief theo vòng, sửa ngay BLOCKING.

## Vòng 1 — 2026-09-22 (trước commit Phase 05)

**Phát hiện tìm được trong quá trình implement (đã sửa + có test):**

- B1 `discovery_service::save` không đăng ký row `sources` trước khi INSERT `mangas` → FK fail với mọi built-in source chưa từng toggle. Sửa: upsert `sources ON CONFLICT DO NOTHING` trong cùng transaction; test `t05_13_14...` assert đúng 1 dòng sources, `t05_16` assert rollback cả sources.
- B2 `madara_generic`: `path_segments` trả segment rỗng cuối do trailing slash → mọi URL `/manga/<slug>/` bị coi là deep path (search trả 0 kết quả); chapter loop gọi nhầm `slug_from_manga_url` (reject chính URL chapter). Sửa: lọc segment rỗng + `chapter_slug_from_url`; 2 test madara xanh.
- B3 `with_transport` sinh id `madara-truyentuoitho.com` (chứa dấu chấm) khác `live()` → test mismatch. Sửa: thống nhất `host.replace('.', "-")`.
- B4 command Tauri không Send vì giữ `MutexGuard` qua `.await`. Sửa: `snapshot()` clone registry ra khỏi lock; add/remove rebuild sau khi xong DB work.

**Phát hiện khi review đối chiếu brief (sửa trong vòng này):**

- F1 (BLOCKING, brief §5.5/§6): Library hiển thị bản ghi đã lưu metadata mà không nói rõ CHƯA tải. Sửa: mỗi card thêm badge `Saved — chưa tải (Phase 06)` + `availableOffline: false`; copy header mới ("Đã lưu = metadata, KHÔNG phải chương đã tải"). Test vitest mới: `labels saved metadata as NOT downloaded`.
- F2 (BLOCKING): `T05-10` redirect re-validation chỉ có trong code, chưa có test. Sửa: thêm `http::tests::redirect_targets_are_revalidated_before_the_next_hop`.
- F3 (BLOCKING): `T05-12` HTML-in-title chưa được assert hành vi. Sửa: vitest `renders hostile provider titles as inert text`.
- F4 (ADVISORY, giữ): copy tooltip "Mở Detail" ghi Phase 05–07 → sửa thành Phase 06–07 cho khớp ranh giới thật ( Detail local chưa có screen).
- F5 (ADVISORY, giữ nguyên): `get_remote_chapters` command tồn tại theo §6 brief nhưng UI dùng `get_remote_manga_details` (một request, đủ dữ liệu). Không xóa (contract), ghi chú tại IPC_CONTRACT.
- F6 (ADVISORY, giữ nguyên): khi TẤT CẢ nguồn lỗi, UI hiển thị danh sách `sourceErrors` (không phải empty-success) nhưng không có nút Retry tổng — user chỉ cần gõ lại query. Chấp nhận cho Phase 05, có thể thêm ở round sau nếu reviewer ngoài yêu cầu.

**Kiểm tra bất biến (đạt):** không `invoke()` ngoài `desktop.ts` (grep); capability/CSP không đổi; một SQLite pool; registry không nhận host từ renderer (Annotation review 4 vòng: chính Phase 05 sau đó thêm `add_manga_source` — người dùng ĐĂNG KÝ host Madara, có validate HTTPS/host đơn/port cấm/IP cấm + GuardedHttp exact-host; bất biến đúng nghĩa là "không host tùy ý ngoài đường đăng ký đã kiểm soát", khai báo trong `docs/IPC_CONTRACT.md`); Demo sandbox tách tab, fixtures không vào DB; migration `0003` verified trên DB thật (schema 3, `app_settings` giữ nguyên, `sources` trống).

**Gate sau vòng 1:** lint 0 · vitest 63/63 · build pass · cargo 45/45 · clippy -D warnings sạch · dev smoke exit 0.

## Vòng 2 — 2026-09-22

Rà lại toàn bộ diff (902 dòng thêm / 23 file sửa + modules mới):

- Wiring đạt: `generate_handler!` đăng ký đúng 9 command discovery (tổng 21); `AppState.registry: Arc<Mutex<SourceRegistry>>`; `sources/providers/fixture.rs` gate `#[cfg(test)]` — không vào binary release.
- Dependency đạt yêu cầu "justified, tối thiểu": `url`, `async-trait`, `percent-encoding` (dùng trong mangadex), `reqwest` (native-tls, không default features), `scraper`, `regex`, `tokio time`. Không crate nào unused (grep xác nhận).
- `save_discovered_manga` không nhận bất kỳ field metadata nào từ UI — chỉ cặp ID; re-fetch verified bằng test.
- Boundary: thêm `.scratch/` vào `.gitignore` (log smoke + copy DB đã xóa khỏi đĩa).
- Không phát hiện BLOCKING mới. Gate giữ nguyên xanh: vitest 63/63, cargo 45/45, clippy -D warnings, lint, build, smoke exit 0 (đã chạy trước vòng 2, code Rust/frontend không đổi sau đó — chỉ docs/.gitignore).
