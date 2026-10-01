# IPC Contract — React ↔ Rust (Phase 04 + 05 + 06 + 07 + 08)

Mọi lời gọi phía frontend đi qua `src/shared/api/desktop.ts` (duy nhất). Tham số và payload serialized **camelCase**; command Rust khai báo snake_case, Tauri tự map. Lỗi trả về luôn là envelope đã kiểm chứng, không bao giờ raw SQL/path/stack.

## Error envelope

```json
{ "code": "DATABASE_INIT_FAILED | VALIDATION_ERROR | NOT_FOUND | DB_CONFLICT | UNSUPPORTED_IN_THIS_PHASE | INTERNAL_ERROR
        | INVALID_URL | UNSUPPORTED_SOURCE | SOURCE_DISABLED | UNSUPPORTED_OPERATION | SOURCE_NOT_FOUND | SOURCE_FORBIDDEN | SOURCE_RATE_LIMITED | SOURCE_NETWORK | SOURCE_PARSE_ERROR | SOURCE_UPSTREAM
        | LIBRARY_ITEM_NOT_FOUND | LOCAL_FILE_MISSING | LOCAL_FILE_CORRUPTED | CBZ_INVALID | UNSUPPORTED_IMAGE_FORMAT | PAGE_RESOURCE_NOT_FOUND | DELETE_FAILED | STORAGE_OUTSIDE_LIBRARY_ROOT",
  "message": "string — an toàn cho UI",
  "retryable": true }
```

Frontend parse qua `normalizeIpcError(err): AppError` (`contracts.ts`). Code lạ → `INTERNAL_ERROR`. `retryable=true`: lỗi DB init/migration và nhóm source Network/RateLimited/Upstream.

## Commands

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `app_get_info` | — | `{ name, version, platform }` | Phase 01 |
| `app_health_check` | — | `{ status, database, schemaVersion }` | Phase 01 + `schemaVersion` (MAX version `_sqlx_migrations`) Phase 04 |
| `app_get_settings` | — | `AppSettingsDto` | SQLite key `settings.v1`; giá trị hỏng → default |
| `app_update_settings` | `{ input: SettingsPatchDto }` | `AppSettingsDto` đã merge+validate | `deny_unknown_fields`; bounds 1–5 / 1–8; enum khóa; lỗi → `VALIDATION_ERROR` |
| `app_reset_settings` | — | `AppSettingsDto` mặc định | |
| `app_list_local_mangas` | `{ query?, limit?, offset? }` | `PagedResult<LocalMangaDto>` | limit 1..=100 (default 20), offset ≤100k; sort title NOCASE + id; query được escape LIKE |
| `app_get_local_manga` | `{ mangaId }` | `LocalMangaDetailDto` | id sai định dạng → `VALIDATION_ERROR`; không tồn tại → `NOT_FOUND` |
| `app_set_manga_favorite` | `{ mangaId, favorite }` | `boolean` | chỉ manga đã đăng ký; idempotent |
| `app_list_favorite_mangas` | `{ limit?, offset? }` | `PagedResult<LocalMangaDto>` | newest first |
| `app_get_reading_progress` | `{ mangaId }` | `ReadingProgressDto \| null` | |
| `app_save_reading_progress` | `{ mangaId, chapterId, pageIndex, scrollOffset?, pageTotal? }` | `ReadingProgressDto` (+`scrollOffset,pageTotal`) | validate ownership chapter∈manga trong MỘT transaction; `0 ≤ pageIndex ≤ 10000`; `scrollOffset∈0..=1`; stamp `mangas.last_read_at` (Phase 07); write lỗi không phá progress cũ |
| `app_list_local_download_tasks` | `{ status?, limit?, offset? }` | `PagedResult<DownloadTaskDto>` | CHỈ ĐỌC (Phase 06 mới ghi); status sai → `VALIDATION_ERROR` |

**Không tồn tại** command chung nào nhận đường dẫn file tuyệt đối, SQL, URL tùy ý để fetch, headers từ renderer hay generic insert metadata. `save_discovered_manga` chỉ nhận cặp ID và tự re-fetch từ provider. **Ba ngoại lệ được khai báo tường minh** (review toàn project 4 vòng; lệnh thứ ba một chiều-vào tương tự): `resolve_manga_url` (URL do CHÍNH người dùng nhập để import — Rust parse + so khớp exact-host theo policy adapter TRƯỚC khi fetch, không fetch URL lạ) và `queue_chapter_download_by_url` (cùng loại ngoại lệ: dán link chương → tải, chỉ nhận chiều vào), `add_manga_source` (base_url người dùng nhập khi đăng ký Madara source — validate HTTPS, host đơn không IP literal/port lạ, sau đó mọi request vẫn qua GuardedHttp exact-host), và chiều NHẬN lại: `outputRelpath`/`local_path` hiển thị chỉ là đường dẫn TƯƠNG ĐỐI theo library root (display-only), không bao giờ path tuyệt đối.

## Commands Phase 05 — source engine & discovery

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `list_supported_sources` | — | `SourceViewDto[]` | Registry động: MangaDex built-in + hàng `sources` adapter `madara` hợp lệ; merge `enabled` từ DB |
| `list_source_genres` | `{ sourceId }` | `GenreTagDto[] { slug, label }` | Danh mục thể loại đọc từ CHÍNH trang nguồn (không có bảng thể loại tự đặt). `slug` là token nguồn dùng trong URL của nó, `label` là chữ nguồn in ra. Nguồn không có trang thể loại → `SOURCE_UNSUPPORTED_OPERATION` và UI ẩn bộ lọc đi thay vì hiện danh sách bịa. Nguồn tắt → `SOURCE_DISABLED` |
| `browse_manga` | `{ sourceId, genre?, status?, sort?, page? }` | `SearchResultDto` (dùng chung shape với `search_manga`) | Trang danh mục có lọc: thể loại và/hoặc trạng thái (`new` / `ongoing` / `completed`) — probe live 2026-09-30 cho thấy một trang thể loại THU hẹp được bằng `?status=0\|2`, nên hai tiêu chí này kết hợp được. `sort` (`newest-desc\|asc`, `updated-desc\|asc`, `views-desc\|asc`) là tính chất RIÊNG của trang thể loại: route trạng thái trả về đúng thứ tự cũ bất kể `?sort=`, nên sort không kèm genre → `VALIDATION_ERROR`. Không chọn genre/status → `VALIDATION_ERROR`; giá trị ngoài tập đóng bị từ chối chứ không mặc định; `slug` thể loại phải khớp `[a-z0-9-]` (chống leo thang đường dẫn). `hasMore` đọc từ chính phân trang `/trang-N` của nguồn |
| `add_manga_source` | `{ displayName, baseUrl, imageHosts? }` | `SourceViewDto { …, imageHosts: string\|null }` | baseUrl phải HTTPS, host public, không port lạ/credential; id do host `madara-<host>`; `imageHosts` (mới, tối đa 4): exact hostname bare (không scheme/port/IP/wildcard) cho phép RIÊNG request ảnh chương của nguồn đó — metadata/search/chapter vẫn chỉ base host; policy fail-closed như cũ; registry rebuild sau ghi |
| `remove_manga_source` | `{ sourceId }` | — | builtin (`mangadex`) → `UNSUPPORTED_IN_THIS_PHASE`; registry rebuild |
| `update_manga_source` | `{ sourceId, displayName, baseUrl, imageHosts? }` | `SourceViewDto` | Sửa tên/tên miền/host ảnh của nguồn HTML — KỂ CẢ built-in (miền hợp đồng: các site hay đổi domain). ID nguồn KHÔNG đổi nên truyện đã lưu vẫn trỏ đúng nguồn; validate qua đúng constructor dựng adapter; từ chối MangaDex (REST host cố định) và va chạm host với nguồn khác (Conflict). Registry rebuild sau lệnh |
| `set_source_enabled` | `{ sourceId, enabled }` | `boolean` | chỉ id có trong registry; tạo row cho builtin khi toggle lần đầu |
| `search_manga` | `{ query, sourceIds?, page? }` | `SearchResultDto` | query 1..=200 ký tự, page 1..=50, ≤10 sourceIds; lỗi từng nguồn cô lập trong `sourceErrors` |
| `resolve_manga_url` | `{ url }` | `RemoteMangaDetailDto` | chỉ URL thuộc adapter đã đăng ký (exact host); URL lạ không bị fetch; `INVALID_URL`/`UNSUPPORTED_SOURCE`/`SOURCE_DISABLED` |
| `get_remote_manga_details` | `{ sourceId, sourceMangaId }` | `RemoteMangaDetailDto` | kèm `savedMangaId` nếu đã có trong catalog; `availableOffline` luôn false ở Phase 05 |
| `get_remote_chapters` | `{ sourceId, sourceMangaId }` | `RemoteChapterDto[]` | cùng nguồn dữ liệu với detail; order ổn định |
| `save_discovered_manga` | `{ sourceId, sourceMangaId }` | `{ mangaId, savedChapterCount, updated }` | re-fetch trusted + MỘT transaction upsert sources→mangas→chapters; không xóa, không đụng favorites/progress |
| `get_home_feed` | `{ sourceId }` | `HomeFeedDto` = `{ featured[], newest[], popular[] }` | Bảng tin Trang chủ, đọc từ CHÍNH trang chủ của nguồn đã kiểm duyệt (TruyenQQ là quyết định sản phẩm cố định). Mỗi item là `RemoteMangaSummaryDto` (flatten) + `latestChapter` — nhãn chương do nguồn in sẵn, KHÔNG suy diễn. Adapter không có trang chủ → `SOURCE_UNSUPPORTED_OPERATION`; nguồn bị tắt → `SOURCE_DISABLED`. Renderer chỉ nhận id + `coverKey` nội bộ, không nhận URL/host nào |
| `get_home_ranking` | `{ sourceId, period }` | `HomeFeedItemDto[]` | Một rail xếp hạng của nguồn theo kỳ; `period` bắt buộc là `day` / `week` / `month` (kỳ lạ → `VALIDATION_ERROR`, KHÔNG âm thầm rơi về `day`), id nguồn phải có trong registry và đang bật (tắt → `SOURCE_DISABLED`). Mỗi item gồm `RemoteMangaSummaryDto` (flatten) + `latestChapter` + `latestTime` nguyên văn từ trang xếp hạng; nguồn không có trang xếp hạng → `SOURCE_UNSUPPORTED_OPERATION`. Host ảnh vẫn do Rust tự ghép từ domain đã duyệt, renderer không nhận URL nào |
| `get_manga_cover` | `{ sourceId, sourceMangaId, coverKey }` | binary `ipc::Response` (ảnh đã validate magic-bytes) | Ảnh bìa Discover/Remote preview. `coverKey` là CHÌA NỘI BỘ do adapter sinh (path không scheme/host — không phải URL fetchable); adapter tự dựng URL đầy đủ qua GuardedHttp exact-host của CHÍNH nguồn (Madara: base host; MangaDex: uploads.mangadex.org fixed; NetTruyen: shard CDN built-in cố định), cap 2 MB, sniff ảnh thật; cache vào `covers/` (regenerable — `clear_regenerable_cache` dọn được). Nguồn bị TẮT → từ chối `SOURCE_DISABLED` trước cả khi đọc cache (chung gate `require_enabled` với mọi luồng discovery). Renderer không bao giờ thấy URL hay path; fail → placeholder, không nới host |

## Commands Phase 06 — download engine (chapter/task id-only)

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `queue_chapter_download_by_url` | `{ url, format? }` | `QueueResultDto` | Tải nhanh từ link chương do CHÍNH người dùng dán (ngoại lệ URL đã khai báo, giống resolve_manga_url): Rust phân loại URL qua adapter đã đăng ký (exact-host + grammar `/manga/<truyện>/<chương>/`), tự LƯU manga qua đúng đường save() nếu chưa có, rồi đi qua CÙNG gate với queue_chapter_downloads (capability Pages, dedup, batch limit). Không URL/path nào chiều ngược lại | `{ chapterIds[] (1..100), format?: "cbz"\|"folder", priority?: -10..10 }` | `QueueResultDto { tasks, reused, errors[], queuedAt }` | KHÔNG nhận URL/headers/path. Format mặc định từ Settings (pdf bị từ chối). Dedup: chapter đã có task non-terminal → `reused`. Lỗi từng mục (`NOT_FOUND`/`UNSUPPORTED_OPERATION` khi nguồn thiếu capability `pages`) không fail cả batch. Một transaction |
| `reveal_download` | `{ taskId? }` | — | Mở Explorer tại nơi lưu tải: có `taskId` (phải `completed`, `output_relpath` được validate relpath + tồn tại trên đĩa) → chọn đúng file/thư mục ảnh trong folder; không có id → mở thư mục library gốc. Đường dẫn chỉ tồn tại phía Rust (hành động, không getter) |
| `get_download_task` | `{ taskId }` | `DownloadTaskDto` | snapshot bền vững; `NOT_FOUND` |
| `pause_download` | `{ taskId }` | `DownloadTaskDto` | queued→paused ngay; `resolving`/`downloading` → flag hợp tác, worker tự chốt `pausing→paused` khi writes dừng; `verifying`/`exporting` → **từ chối bằng `VALIDATION_ERROR`** (cửa sổ commit ngắn, pause không thể giữa chừng — review 4 vòng); transition sai khác cũng `VALIDATION_ERROR` |
| `set_download_queue_paused` | `{ paused: bool }` | `u32` (số dòng status bị chuyển hàng loạt) | Cổng hàng đợi toàn cục: pause chặn dispatcher giành task mới, cooperative-pause mọi worker đang chạy và chuyển queued->paused; resume làm ngược lại (kể cả task người dùng tạm dừng riêng lẻ cũng được resume theo thiết kế). Cờ gate chỉ sống trong bộ nhớ — khởi động lại app luôn bắt đầu chạy tiếp |
| `get_download_queue_paused` | `()` | `bool` | Đọc trạng thái cổng hàng đợi cho UI |
| `resume_download` | `{ taskId }` | `DownloadTaskDto` | chỉ paused→queued |
| `cancel_download` | `{ taskId }` | `DownloadTaskDto` | queued/paused ngay; worker nhận flag tự chốt cancelled; staging của task bị xóa, **không đụng file đã publish** |
| `retry_download` | `{ taskId }` | `DownloadTaskDto` | chỉ failed→queued; giữ attempts/backoff đã lưu |
| `set_download_priority` | `{ taskId, priority }` | `DownloadTaskDto` | chỉ queued/paused/failed |
| `remove_download_task` | `{ taskId }` | `boolean` (đã xóa row hay không) | Gỡ MỘT dòng đã kết thúc khỏi danh sách tải — cùng luật với `clear_download_history`: chỉ `completed`/`failed`/`cancelled`, không bao giờ chạm file. `completed` còn artifact trên đĩa → `VALIDATION_ERROR` (row là bằng chứng offline, phải xóa qua Thư viện). Task đang chạy → `VALIDATION_ERROR`; id lạ → `NOT_FOUND`. `download_pages` cascade theo row. UI: card `failed` chỉ còn "Thử lại" + "Gỡ khỏi danh sách" — nút "Hủy" cũ bị backend từ chối nên đã bỏ |
| `clear_download_history` | `{ statuses?: ("completed"\|"failed"\|"cancelled")[], olderThanDays?: 0..=36500 }` | `ClearHistoryDto { deleted, skippedArtifact, skippedOther }` | Feature plan F2. CHỈ xóa dòng lịch sử terminal — không bao giờ xóa file. Completed còn artifact trên đĩa → `skippedArtifact` (row là bằng chứng offline). Guard `status IN (terminal)` ngay trong DELETE: task vừa được retry giữa chừng không bị xóa. `download_pages` theo cascade. Status ngoài terminal set / rỗng → `VALIDATION_ERROR` |
| `retry_all_failed_downloads` | — | `number` (số task thực sự requeue) | Feature plan F5. FIFO theo `created_at`, cap 200/lần; mỗi row đi qua đúng guarded retry của `retry_download` (race-safe); notify dispatcher một lần |
| `cancel_downloads_for_manga` | `{ mangaId }` | `number` (số tác vụ accepted) | Feature plan F5. Mọi task chưa xong (trừ terminal/failed) của mọi chương thuộc manga; queued/paused chốt ngay, worker-active nhận cancel flag hợp tác. Kết quả cuối cùng qua event — con số là "đã nhận yêu cầu", không phải "đã cancelled" |
| `app_list_local_download_tasks` (mở rộng) | `{ status?, limit?, offset? }` | `PagedResult<DownloadTaskDto>` | status nay nhận cả 10 lifecycle states; DTO thêm `attempts/nextRetryAt/errorMessage/revision/finishedAt/outputRelpath/chapterTitle/mangaTitle` |

## Commands Feature plan F1 — new-chapter watch (schema 6 `0006_watch.sql`)

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `library_get_watch` | `{ mangaId }` | `{ enabled: bool\|null, watchedAt: string\|null, failCount }` | `enabled: null` = theo cài đặt chung `settings.watchEnabled` |
| `library_set_watch` | `{ mangaId, enabled: bool\|null }` | — | Bật/tắt theo dõi từng truyện; `null` khôi phục theo chung; `NOT_FOUND` nếu manga lạ |
| `library_list_alerts` | `{ mangaId }` | `ChapterAlertDto[] { chapterId, title, detectedAt }` | Tối đa 200, chưa ack; metadata thuần, không URL/path |
| `library_ack_alerts` | `{ mangaId }` | `number` (xóa bao nhiêu) | Idempotent |
| `watch_run_now` | — | `{ checked, alertedChapters, failed }` | Manual tick CƯỠNG ÉP (bỏ cửa sổ due — review vòng 1; loop nền ~6h vẫn tôn trọng backoff từng truyện); rate-limit 60s CAS → `DB_CONFLICT`; dùng chung `perform_tick` với loop |

| `app_list_read_history` | `{ mangaId?, limit (1..100), offset }` | `PagedResult<ReadHistoryDto { mangaId, mangaTitle, chapterId, chapterTitle, chapterNumber, readAt }>` | Feature plan F4 (schema 7 `read_history` — UPSERT trong cùng transaction save_progress khi đến 2 trang cuối). Xóa lịch sử theo manga = metadata-only, không động file/progress/favorites |
| `app_clear_read_history` | `{ mangaId }` | `number` (số xóa) | idempotent |
| `search_history_record` | `{ query }` | `RecentSearchDto[]` (danh sách mới, mới nhất trước) | Discover Phase 2 (schema 10 `recent_searches`). CHỈ lưu đúng những gì người dùng nhập: không kết quả, không sourceId, không bộ lọc. Trim + gộp khoảng trắng; rỗng hoặc > 80 ký tự → `VALIDATION_ERROR` (không ghi xuống bảng). UPSERT theo query nên tìm lại không nhân bản, và mỗi lần ghi cắt bảng còn `MAX_ENTRIES = 10` dòng mới nhất. Trả về danh sách đã lưu để renderer không tự suy |
| `search_history_list` | — | `RecentSearchDto { query, searchedAt }[]` | đọc thuần |
| `search_history_remove` | `{ query }` | `RecentSearchDto[]` | xoá đúng một truy vấn; trả danh sách mới |
| `search_history_clear` | — | `number` (số xóa) | idempotent; chỉ động bảng `recent_searches` |

`library_list` (mở rộng): DTO thêm `newAlerts` (số chương mới chưa ack), filter mới `"alerts"`. **Event mới** `library-alerts` payload `{ newChapters }` — Rust chỉ emit khi tick phát hiện chương mới (kèm 1 notification tổng); frontend bump domain `library`/`home`, dữ liệu luôn refetch qua snapshot. Watcher CHỈ gọi `manga_detail` qua adapter (đúng pacing/allowlist GuardedHttp), không tự HTTP, không download gì.

**Event** `download-task-updated` → payload `DownloadTaskEvent { task, downloadedBytes, speedBytesPerSecond|null, occurredAt }`: progress gộp ≥250ms/task, chuyển trạng thái tức thì; `task.revision` tăng đơn điệu để UI chặn frame cũ ghi đè frame mới. Frontend subscribe qua `desktop.subscribeDownloadTask()` và **bắt buộc unlisten khi teardown**. `DownloadTaskDto` không bao giờ chứa URL trang/headers/đường dẫn tuyệt đối; `outputRelpath` là đường dẫn tương đối dưới app library root, chỉ hiển thị, không phải input.

## Commands Phase 07 — library & offline reader (id-only, binary per page)

| Command | Input (camelCase) | Output | Ghi chú |
|---|---|---|---|
| `library_list` | `{ query?, sort?('lastRead'│'recentDownload'│'title'│'chapters'), filter?('all'│'downloaded'│'partial'│'favorite'│'alerts'│'reading'│'completed'│'missing'), genre?, limit(1..100), offset }` | `PagedResult<LibraryMangaDto>` | aggregate 1 query (counts, size, progress, hasMissing) — không N+1; query escape LIKE. Library redesign: thêm `genres[]` + `description` trên mỗi dòng và `genre` (lọc qua `manga_genres`, EXIST nên không nhân bản dòng); ba filter mới `reading`/`completed`/`missing` chính là bộ đếm ở "Bộ lọc nhanh" — test `t11_quick_filters_match_the_counts_they_display` buộc hai bên khớp nhau |
| `library_genres` | — | `LibraryGenreDto[] { genre, mangaCount }` | Thể loại CÓ TRONG THƯ VIỆN local, đếm từ `manga_genres` của chính app (không phải số liệu nguồn). Rỗng → UI ẩn pill, không hiện danh sách bịa |
| `library_get_downloaded_chapters` | `{ mangaId }` | `{ chapters: DownloadedChapterDto[] }` | `artifactOk` = Rust fs-check tại lúc gọi |
| `library_reconcile` | — | `{ checked, healthy, missing, corrupted }` | phi phá hủy: missing⇒clear cờ offline, corrupted⇒clear cờ + GIỮ file; không bao giờ xóa |
| `library_remove_chapter` | `{ chapterId }` | — | path lấy từ DB, validate relpath + under library root; **xóa disk trước, DB sau**; fail ⇒ `DELETE_FAILED`, trạng thái nguyên |
| `library_remove_manga_files` | `{ mangaId }` | `{ removed[], failed[] }` | per-item trung thực; giữ metadata/favorites/progress |
| `library_import_cbz` | `{ mangaId, chapterNumber?, chapterTitle? }` | `ImportedChapterDto \| null` (null = hủy) | Feature plan F3b. **Dialog native mở từ RUST** — renderer không gửi/nhận đường dẫn; file chọn được parse qua `cbz::open` UNTRUSTED (chống traversal + cap trang + zip-bomb deflate), copy vào library root với tên sinh từ UUID đã sanitize (không dùng tên nguồn), rồi ghi `chapters` (+`local_path/output_format/page_count/size`) và `download_tasks` `completed` nhân danh import trong MỘT tx. Không capability `dialog:` mới (chỉ API phía Rust, JS không gọi). Trả về id + metadata, không path |
| `library_import_cbz_path` | `{ mangaId, path }` | `ImportedChapterDto` | Kéo-thả CBZ: đường dẫn đến từ sự kiện drop native của webview (ngoại lệ có chủ đích của quy tắc dialog-mở-từ-Rust). Rust chặn cứng phần mở rộng `.cbz`, rồi dùng NGUYÊN luồng xác minh untrusted của import_cbz (traversal, zip-bomb, chỉ ảnh, cap dung lượng, copy tên UUID) — không gì ngoài id + metadata trả về |
| `reader_open_chapter` | `{ chapterId }` | `ReaderManifestDto` | chỉ chapter có `local_path`; `LOCAL_FILE_MISSING`/`CBZ_INVALID`/`LOCAL_FILE_CORRUPTED`; manifest KHÔNG chứa path/tên file; kèm prev/Next chương đã tải + startPage resume. **F3a:** CBZ hỗ trợ cả STORED lẫn DEFLATE (flate2) — entry method lạ ⇒ `CBZ_INVALID`; inflate có cap `MAX_PAGE_BYTES` khi mở VÀ chặn `total_out` khi phình quá khai báo (chống zip-bomb); reader của app và `reconcile` dùng chung `cbz::open` nên CBZ deflate ngoài nhập vào cũng reconcile healthy |
| `reader_get_page_bytes` | `{ chapterId, pageIndex }` | **binary** (`ipc::Response` → JS `ArrayBuffer`) | 1 trang/lần, cap 20MB; entry archive là basename-only; magic-check; `PAGE_RESOURCE_NOT_FOUND` |

`app_list_local_download_tasks`… (Phase 06 giữ nguyên). Reader page: UI dùng `URL.createObjectURL` blob có thu hồi; CSP `img-src` thêm `blob:` (duyệt trong Phase 07); capabilities không đổi.

## Commands Phase 08 — integration, storage & Windows (id-only, approximate sizes)

| Command | Input | Output | Ghi chú |
|---|---|---|---|
| `library_stats` | — | `LibraryStatsDto { savedMangas, totalChapters, downloadedChapters, downloadedMangas, localSizeBytes, favorites, reading, completed, paused, missing }` | 1 SQL aggregate cho Home + "Bộ lọc nhanh" của Thư viện; không N+1. `downloadedMangas` là số TRUYỆN có ≥1 chương offline (khác `downloadedChapters`), `paused` đếm chương trình trong hàng đợi nên UI diễn đạt bằng chữ, không gọi nó là "truyện tạm dừng" |
| `storage_status` | — | `StorageStatusDto { libraryBytes, cacheBytes, stagingDirs, activeTasks, rootAvailable, approximate:true, customRoot, rootLinkOk, volumeTotalBytes\|null, volumeFreeBytes\|null }` | kích thước từ DB + walk root app-owned; symlink bỏ qua; **không absolute path**. `volume*Bytes` là dung lượng Ổ THẬT của volume chứa thư viện (`GetDiskFreeSpaceExW` khai báo FFI tay, không thêm dependency; junction được đi theo nên đúng ổ người dùng chọn). `null` khi ngoài Windows hoặc gọi lỗi → UI bỏ thanh dung lượng, **không thay bằng quota bịa** |
| `clear_regenerable_cache` | — | `CacheCleanupDto { removedDirs, freedBytes, keptActiveDirs }` | chỉ staging của task terminal/unknown + covers; active-set chặn tuyệt đối; idempotent; cũng chạy tự động ở startup sau recovery |

**Settings keys mới** (`AppSettingsDto`): `notificationsEnabled` (bool, default true) — Rust notify theo chapter-terminal; `closeToTray` (bool, default false) — X ẩn cửa sổ. Validate trong `domain/settings.rs`; `RuntimeFlags` mirror cho sync path; command save/reset đồng bộ flag sau persist. UI bỏ option `pdf` (engine vẫn từ chối pdf tại queue như cũ).

**Event** `tray-open-downloads` → payload rỗng: tín hiệu điều hướng từ tray menu; subscribe qua `desktop.subscribeTrayOpenDownloads()`, unlisten khi teardown (bridge duy nhất cấp app).

**Invalidation frontend:** `src/shared/hooks/useInvalidation.ts` — 6 domain transient (`home/library/favorites/downloads/settings/sources`; domain `progress` đã gỡ vì không có bump/subscriber — review toàn project 4 vòng); event/status mutation CHỈ bump domain, screens refetch snapshot từ Rust. Không phải kênh dữ liệu mới — không thêm IPC.

Capabilities KHÔNG đổi (notification gọi từ Rust, không qua IPC permission).


## Commands — Nơi lưu truyện đã tải (2026-09-25, user duyệt đổi policy root)

| Command | Input | Output | Ghi chú |
|---|---|---|---|
| `get_download_root` | — | `{ isCustom, label, linkOk }` | label: đường dẫn user đã chọn (chỉ hiện ở Settings, text thuần) hoặc chuỗi mặc định |
| `choose_download_root` | — | `DownloadRootDto \| null` | Hộp chọn thư mục mở TRONG Rust (khuôn F3b — không capability `dialog:`); null = hủy. Quy trình: validate (tuyệt đối, không gốc ổ đĩa/hệ thống/trong appdata, không phải liên kết) → copy + so-sánh từng byte → junction `appdata\library → <đã chọn>\library` → lưu key `storage.downloadRoot.v1`; lỗi ở bước nào rollback bước đó, không xóa nguồn trước khi verify xong |
| `reset_download_root` | — | `DownloadRootDto` | Copy ngược về app-data rồi bỏ junction; thư mục user từng chọn KHÔNG tự xóa |

`storage_status` mở rộng `customRoot`, `rootLinkOk` — link hỏng không bao giờ được báo cáo là "library trống". Cache/staging (tệp tạm, covers) luôn ở app-data, chỉ library di chuyển. `chapters.local_path`/`download_pages.storage_key` là đường dẫn TƯƠNG ĐỐI nên engine/reader/reveal/import trong suốt qua junction, không cần migration.

## DTO định nghĩa tại `src/types/backend.ts`

`LocalMangaDto` (kèm `chapterCount`, `favorite`), `LocalChapterDto` (+`downloaded`, `outputFormat` — badge offline do Rust kiểm chứng), `LocalMangaDetailDto`, `ReadingProgressDto`, `DownloadTaskDto`, `AppSettingsDto`, `PagedResult<T>`. Phase 06 thêm `src/types/downloads.ts` (`QueueResultDto`, `DownloadTaskEvent`, bảng transition UI, format helpers) và `localServices.downloads` (`src/services/downloads.api.ts`).

## DTO Phase 05 tại `src/types/source.ts`

`SourceViewDto` (flatten `SourceInfo` + `enabled/builtin/baseUrl`), `RemoteMangaSummaryDto`, `RemoteChapterDto`, `RemoteMangaDetailDto` (flatten summary + `chapters`, `savedMangaId`, `availableOffline`), `SearchResultDto` (`items`, `sourceErrors[]`, `hasMore`), `SaveResultDto`.

## Mô hình tiêu thụ phía UI

- `localServices.library` (`src/services/local-library.api.ts`) — màn **Local**: Library, Favorites.
- `localServices.source` (`src/services/source.api.ts`) — **Production discovery** (Phase 05): Discover live tabs, `/remote/...`, Settings → Sources.
- `localServices.downloads` (`src/services/downloads.api.ts`) — **Production queue** (Phase 06): Downloads manager, batch submit, event `download-task-updated`.
- `localServices.libraryV2` / `localServices.reader` (`library.api.ts` / `reader.api.ts`) — **Production library + offline reader** (Phase 07): Library/LocalManga/`/read/:chapterId`.
- `services` (`provider.ts`) — sandbox **Demo**: Manga Detail/Downloads/Reader/Home + tab Demo sandbox (mock fixtures, nhãn Demo).
- `services.settings` — đã là adapter THẬT (`settings.api.ts`) từ Phase 04.
- Quy tắc: DB error ≠ empty library; provider error ≠ no results. UI phải phân biệt `ErrorState` (retry) / danh sách `sourceErrors` / `EmptyState`.
