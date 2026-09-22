# Phase 05 Integration Notes — map & decisions

> Bản GIỮA TRẠNG THÁI: mục "Map" và "Kiến trúc Rust mới" bên dưới là kế hoạch ban đầu; kết quả thực tế (9 command, registry động trong `Arc<Mutex>`, provider `madara_generic`, frontend tabs/saved) được cập nhật trong `IPC_CONTRACT.md`, `ARCHITECTURE.md` và `test-results.md`.

## Map dịch vụ Phase 03/04 → Phase 05

| Boundary | Phase 04 | Phase 05 | Ghi chú |
|---|---|---|---|
| Discover — tab Search | mock fixtures (demo) | **tab mới `Live search`** qua `search_manga` (MangaDex) | Demo giữ nguyên, tách tab riêng, nhãn rõ |
| Discover — Import URL | mock regex `demo.manga1989.example` (demo tab) | **tab `Import URL (live)`** qua `resolve_manga_url` (chỉ host đã đăng ký) | Demo import vẫn ở demo tab |
| Manga Detail `/manga/:mangaId` | fixture demo + Local empty state | **trước tiên tra SQLite thật** (`app_get_local_manga`) → chế độ Local/DB; fallback fixture → vẫn nhãn demo; khác → NotFound | Một route, hai chế độ phân biệt bằng badge + copy |
| Remote detail (chưa lưu) | — | route mới **`/remote/:sourceId/:sourceMangaId`** (explicit, deep-link được, refresh an toàn) — preview từ provider + nút **Save to Library** | §6 cho phép route mới; không trộn vào `/manga/:id` |
| Library | Local DB (trống tới giờ) | dữ liệu thật sau Save metadata; mỗi card ghi trạng thái "Saved — chưa tải (Phase 06)" | `availableOffline` luôn false ở Phase 05 — không được suy tôn từ metadata |
| Settings/Sources | mock `enabledSourceIds` | registry thật `list_supported_sources` + `set_source_enabled` (ghi DB `sources.enabled`); section demo nguồn tách riêng, nhãn rõ | Bỏ dần mock source state khi UI đã chuyển hết sang IPC |
| Downloads/Reader | mock | giữ nguyên demo — mọi nút download/read thật **disabled + giải thích Phase 06/07** | §1.3 |
| `services.library` (demo favorites) | mock | giữ (demo sandbox) — MangaDetail ở **chế độ Local** dùng IPC `app_set_manga_favorite` (đã có từ Phase 04) | Hai chế độ, hai nguồn state, một màn hình |

## Kiến trúc Rust mới

```
src-tauri/src/sources/
  mod.rs        — SourceRegistry (Arc adapter), build production registry = [MangaDex]
  dto.rs        — SourceInfo/Capability, RemoteMangaSummary/Detail/Chapter, SearchPage
  adapter.rs    — trait SourceAdapter (async-trait) + SourceError taxonomy
  http.rs       — GuardedFetcher: HTTPS + exact-host allowlist + port mặc định,
                  IP riêng/link-local/reserved bị chặn cả literal lẫn đã resolve,
                  redirect tự follow ≤3 hop (mỗi hop re-validate),
                  size cap ~2 MB, timeout 15s, UA định danh,
                  trait Transport để mock trong test
  resolver.rs   — parse URL người dùng → tìm adapter sở hữu (exact host + path pattern)
  providers/mangadex.rs — ops read-only: search/detail/feed; map JSON→dto
  providers/fixture.rs  — deterministic, test-only registry injection
services/discovery_service.rs — fan-out search cô lập lỗi theo nguồn; resolve/preview;
                  save: fetch lại từ provider ĐÁNG TIN CÂY (không tin payload UI),
                  transaction upsert source+manga+chapter, trả {mangaId, savedChapterCount, updated}
commands/discovery.rs — 7 lệnh §6 brief, input bounds + revalidate mọi sourceId
```

State: `AppState { db, registry: Arc<SourceRegistry> }` — vẫn **một pool** duy nhất.

## Quy tắc an toàn đã chốt

- Renderer không bao giờ gửi host/URL tùy ý để fetch: chỉ `query`, `sourceId`, `sourceMangaId` hoặc URL để **đối khớp** registry.
- `save_discovered_manga` re-fetch từ provider; bỏ qua mọi field do UI gửi ngoài cặp ID.
- Không xóa chapter khi provider response thiếu; refresh chỉ add/update; favorites/progress/task refs không đụng.
- `Retry-After`/429: trả `PROVIDER_ERROR` kèm thông điệp, không tự retry vòng lặp.
- HTML trong title/description render text thuần (React mặc định) — cấm `dangerouslySetInnerHTML`.
- Capability/CSP: **không đổi** (HTTP trong Rust; webview không gọi remote).

## Phụ thuộc Rust thêm mới ( justified, tối thiểu)

`reqwest` (blocking-free: default rustls? dùng native-tls Windows/Schannel), `url`, `async-trait`, `ipnet`? — thay bằng `std::net::IpAddr` tự kiểm phạm vi, không `ipnet`. Cho phép: reqwest, url, async-trait.
