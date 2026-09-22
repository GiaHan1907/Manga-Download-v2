# Phase 04 Self-Review

**Vòng 1** — 2026-09-22, sau commit P04 (trước khi chốt smoke).

## Bằng chứng độc lập với lời khai trong notes

| Kiểm | Kết quả |
|---|---|
| `git diff 5888d5a -- src-tauri/capabilities` | **rỗng** — không quyền mới (T04-14) |
| Đếm `#[tauri::command]` trong `commands/*` | 12 = đúng số đăng ký trong `generate_handler!` (không command "ma") |
| grep `invoke(` ngoài `desktop.ts` | chỉ `desktop.ts` chứa `invoke<` — boundary giữ nguyên |
| grep `query!|query_as!` macro SQLx | 0 — mọi query runtime, không DATABASE_URL build-time |
| grep `fetch(|reqwest|shell|Command::new` | 0 trong Rust src mới — không đường dẫn network/process |
| Test DB trỏ tempdir | mọi `test_pool()/init_pool` dùng `tempfile::tempdir` — DB user không bị test chạm |
| `0001_init.sql` | không đổi (git log 1 commit) |

## Findings vòng 1

| # | Finding | Phân loại | Xử lý |
|---|---|---|---|
| R1 | `MangaDto` sinh ra thiếu đường đọc favorite flag cho UI Local → star Library không phản ánh trạng thái DB | BLOCKING (đã sửa ngay trong lúc code, kiểm lại ở đây) | EXISTS(...) AS favorite trong `MANGA_SELECT`, DTO 2 phía; 23/23 + 51/51 pass |
| R2 | Test cũ `mockServices.test` vẫn gọi settings qua provider (nay là adapter IPC thật) → fail | Đã sửa | trỏ thẳng `mockSettingsService` (sandbox vẫn cần cho demo screens) |
| R3 | `settings.api.save` ném `Error(message)` mất `code` (UI chỉ hiển thị message) | ADVISORY | Chấp nhận cho Phase 04: message đã sanitize; cải tiến envelope đầy đủ khi có nhu cầu phân nhánh UI |
| R4 | Library card Local không hiện ảnh bìa (schema có `cover_key` nhưng chưa có đường serving) | ADVISORY | Đúng ranh giới Phase 04 — serving thuộc Phase 05–06; đã ghi chú UI |
| R5 | "DB locked/unavailable" phía Rust chưa mô phỏng mức repository (busy_timeout mới là mitigation, chưa có test va chạm 2 process) | ADVISORY | Hành vi mức UI đã test (retry/recover); ghi vào known-limitations, cân nhắc test 2-connection khi làm Phase 06 |
| R6 | `npm run tauri build` chưa chạy lại sau khi code Rust đổi | BLOCKING cho **release**, không phải cho phase gate | Kế hoạch: chạy lại trước khi đóng release (đã ghi test-results §1) |

## Bằng chứng smoke live (bổ sung sau vòng 1)

tauri dev exit 0; migration 0002 áp dụng thành công lên DB user đã tồn tại (nâng cấp thật, không reset); `covers/`+`cache/` được tạo. Xem `test-results.md` §2.

## Kết luận vòng 1

Không còn BLOCKING mở trong phạm vi phase (R6 thuộc gate release). DoD §8 của brief: tất cả mục đạt với bằng chứng ở `test-results.md`; 2 deviations có giải trình: (1) không thêm React Query/Zustand — dùng `useAsyncData` + `useSyncExternalStore` sẵn có theo rule "existing conventions take precedence" của chính brief; (2) `docs/ARCHITECTURE.md` + `docs/IPC_CONTRACT.md` được **tạo mới** trong phase này vì chưa tồn tại.
