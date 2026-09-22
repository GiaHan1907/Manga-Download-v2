# Phase 09 — Security Review

**Commit được soi:** `71c5ba8` + các test P09 trên working tree này. **Runner:** Windows 11 Pro x64 (Build 26200).

## 1. Attack surfaces & controls (mỗi dòng đối chiếu code/test thật)

| Surface | Control (code) | Bằng chứng test | Kết luận |
|---|---|---|---|
| URL import / search / source adapter | `GuardedHttp`: HTTPS-only, exact-host allowlist, port 443-only, chặn userinfo/IP-literal; redirect tự-follow + **re-validate từng hop**; `assert_resolved_addresses_public` chặn loopback/private/link-local/metadata/CGNAT/ULA cả khi DNS trả về | `rejects_non_https_and_foreign_hosts`, `rejects_credentials_and_ip_literals_and_private_targets`, `cgnat_and_ula_ranges_are_blocked`, `redirect_targets_are_revalidated_before_the_next_hop` (P05) + **MỚI `t09_dns_guard_rejects_private_literals_and_unresolvable_hosts`** (guard DNS trả-private + NXDOMAIN fail-closed) | SSRF chặn ở tầng mở kết nối, không chỉ ở parser |
| Payload từ nguồn | size caps `MAX_BODY_BYTES` 2MB / `IMAGE_MAX_BYTES` 20MB / `MAX_PAGE_COUNT` 300; classify 401/403/404/429(+Retry-After)/5xx; timeout 15s; pacing ≥3s/host | **MỚI `t09_classify_status_maps_http_codes_and_retry_after`**, `t09_per_host_pacing_enforces_minimum_spacing` (đo thật ≥2.6s), `t09_mock_byte_transport_enforces_size_cap`, `script_404_429_500_parse_map_to_distinct_errors` | Lỗi nguồn phân loại đúng, không vô hạn retry, không hammer |
| Ảnh tải về | magic-bytes-first (`detect_image_kind`), extension do kind quyết định, HTML/truncated/oversized reject, CRC32 checkpoint | `t06_08_*`, `valid_image_passes_and_kind_drives_extension`, `crc32_matches_known_vectors`, `html_body_never_lands_as_staged_page` | Ảnh giả MIME không vào staging |
| CBZ đọc offline | EOCD tail-window + central-segment **có cap** (`entry_count ≤ 10_000`, `central_len ≤ 8MB`), entry name basename-only, `MAX_PAGE_BYTES` 20MB cap **trước khi đọc**, STORE-only, image magic re-check mỗi trang trả về | `entry_name_guard`, `t07_missing_corrupt_and_traversal_paths_are_controlled`, **MỚI 7 test `t09_*` trong `reader/cbz.rs`** (phễu thủ công: deflate→CBZ_INVALID, traversal reject-at-open, declared-size quá cap reject-trước-read, central truncation, count-cap >10k, non-image skip, empty→FileCorrupted) + `read_page_caps_size_and_rejects_unsafe_names` (folder) | Zip-bomb-class + traversal chặn cả ở open lẫn read; cbz.rs từ 0 → 7 unit test |
| IPC abuse | Renderer chỉ gửi ID/index/limit/offset; `validate_id` + `validate_page` (limit 1..=100, offset ≤100k) + `deny_unknown_fields` settings; binary trả `ipc::Response` (ArrayBuffer) | `catalog_service::oversized_query_and_malformed_ids_rejected`, `t04_09`, `t06_04`, settings tests, `t04_07_wrong_pairing...` | Không có lệnh generic read/delete/fetch/execute nào tồn tại (registry 38 commands, tất cả typed) |
| Filesystem scope | Mọi path từ DB relpath → `validate_relpath` → `ensure_under_root` (canonicalize + containment; symlink/reparse không thoát); delete disk-first; reconcile non-destructive; cleanup chỉ staging terminal + covers (kind-only logging) | `ensure_under_root_blocks_symlink_escape`, `relpath_rejects_traversal_and_absolute`, `t07_remove_*`, `t08_clear_cache_keeps_active_and_removes_terminal_orphans`, **MỚI `t09_unicode_titles_produce_safe_path_components`** | Không thao tác nào nhận path từ renderer; Unicode/reserved an toàn |
| Metadata render | React text nodes; KHÔNG `dangerouslySetInnerHTML`; palette/reader/search hiển thị title qua JSX | vitest `phase05 hostile titles inert`, `phase08` palette XSS assertion (`no img from title`) | HTML/script trong title là text vô hại |
| Capabilities / CSP | `capabilities/default.json`: core:default + 5 window perms; **không fs/shell/http/opener/notification-JS** (notification gọi từ Rust nên không cần thêm perm); CSP `default-src 'self'`, img `blob:` thêm có kiểm soát (Phase 07) | File đọc trực tiếp tại review; smoke `tauri dev`/`build` chạy với đúng file này | Không wildcard; không mở rộng để chữa cháy |
| Logging | eprintln chỉ `kind()`/mã lỗi/wire-pair code — **MỚI**: sửa 2 chỗ in `io::Error` Display đầy đủ (library delete, staging cleanup) + lib.rs prepare; wire errors sanitize qua `wire_pair()` | Grep toàn repo `eprintln!` tại review (4 site, tất cả path-free sau sửa) | Không lộ absolute path/private URL trong log |
| Dependencies | `npm audit` (all): **0 vulnerabilities**. `cargo audit`: 1 advisory — `rsa 0.9.10` RUSTSEC-2023-0071 (Marvin, timing, medium): `cargo tree --target x86_64-pc-windows-msvc -i rsa` = **empty** → crate không xuất hiện trong build Windows (entry lockfile đa nền tảng của resolver); 8 warning "unmaintained" đều transitive build-time/platform khác — **triage: không action, không major-bump dây chuyền** | Lệnh + output lưu tại `evidence/audit-2026-09-23.txt` | Không finding nào chạm binary shipped |

## 2. Findings & xử lý

| # | Finding | Severity | Xử lý |
|---|---|---|---|
| S-1 | `reader/cbz.rs` không có unit test adversarial trực tiếp (chỉ service-level) | Medium (test gap) | **Đã sửa:** 7 test builder-thủ công ở trên |
| S-2 | `eprintln!` in `io::Error` Display (có thể chứa path trên Windows recursive ops) | Low | **Đã sửa:** log `kind()` tại library_service + storage_service + lib.rs |
| S-3 | Testkit `png_bytes(index as u8 + 1)` overflow panic với fixture >255 trang (debug) | Low (test-only) | **Đã sửa:** `(index % 250) + 1` — lộ ra khi perf test 300 trang |
| S-4 | Pacing/`classify_status`/DNS-guard từng là code không test | Medium (test gap) | **Đã sửa:** 3 test t09 mới |
| S-5 | `fetch_bytes_bounded` cap phía `GuardedHttp` thật chỉ đọc-code-được, không test end-to-end (cần local HTTPS server) | Low | Chấp nhận + ghi release-risk Phase 10; semantics cap được chứng minh qua MockTransport cùng contract |
| S-6 | rsa RUSTSEC-2023-0071 trong cargo audit | Advisory-only | Triaged: không có trong Windows tree (xem bảng trên) — ghi release note |

## 3. Không tìm thấy (đã chủ động soi)

- Không `dangerouslySetInnerHTML`, không `eval`, không `asset:` scope rộng, không `shell.open`/opener, không `fs` plugin, không command nào nhận path/URL tùy ý, không secret/URL private trong notify body (chỉ title metadata), không wildcard CSP.

## 4. Kết luận

Không còn finding **Critical/High** chưa xử lý. Các remediation là test-gap + logging hygiene; không mở rộng capability, không đổi contract dữ liệu.
