# Phase 01 — Nền tảng Manga Downloader 1989

## Bối cảnh

**Lý do:** Repo `D:\Project\Mangav2` đang trống (chỉ có `AGENTS.md`). Phase 01 tạo nền móng cho ứng dụng desktop Tauri 2 local-first, để các phase sau có thể thêm: import URL manga, tìm kiếm trong app, hàng đợi tải chương, thư viện, và trình đọc offline. Nếu không có scaffold này, không thể build bất kỳ tính năng nghiệp vụ nào.

**Nguyên nhân:** User cung cấp brief `PHASE_01_IMPLEMENTATION_PLAN.md` (trên Desktop) mô tả chi tiết kiến trúc, cây thư mục đích, dependencies, IPC contracts, SQLite migration, ranh giới bảo mật, và 8 task theo thứ tự. Plan này dịch brief đó thành các bước thực thi cụ thể cho Qoder agent.

**Kết quả mong muốn:** Một app desktop Tauri 2 + React + TypeScript + Vite + Rust/SQLx chạy được trên Windows x64, với:
- Cửa sổ shell dark-purple mở lên
- Hai lệnh IPC typed (`app_get_info`, `app_health_check`) gọi qua một wrapper duy nhất
- SQLite khởi tạo tại `%APPDATA%\com.manga1989.downloader\` với migration nhúng `0001_init.sql` (chỉ tạo bảng `app_settings`)
- Pass lint, typecheck, frontend test, Rust test, Vite build, và `cargo check`
- Tauri capabilities chặt (không có plugin fs/shell/http) và CSP chặt (không wildcard/remote script)
- README mô tả prerequisite, lệnh chạy, cấu trúc, vị trí DB, và giới hạn phạm vi Phase 1

**KHÔNG có tính năng nghiệp vụ:** không crawler, không import URL, không tìm kiếm trong app, không hàng đợi tải, không thư viện, không trình đọc offline, không bảng manga/chapter/source/download trong DB. Đó là Phase 2+.

---

## Quyết định đã chốt

| Quyết định | Giá trị | Lý do |
|---|---|---|
| Bundle identifier | `com.manga1989.downloader` | User đã confirm. Reverse-DNS, quyết định đường dẫn data `%APPDATA%\com.manga1989.downloader\` |
| Tên package npm | `manga-downloader-1989` | Theo brief §5 |
| Tên hiển thị app | `Manga Downloader 1989` | `tauri.conf.json` → `productName` |
| Port dev Vite | `1420`, `strictPort: true` | Mặc định Tauri; ví dụ trong brief §5 |
| Git init | **Có**, commit `AGENTS.md` + `.gitignore` làm baseline trước khi scaffold | User đã confirm. Brief yêu cầu commit lockfiles; workflow AGENTS.md dùng `git diff/status` |
| Versions | Chọn bản stable mới nhất tương thích tại thời điểm scaffold (Tauri 2.x, React 19.x, Vite 7.x hoặc 6.x, SQLx 0.8.x); khóa qua `package-lock.json` + `Cargo.lock` | Brief §3.6 — không hardcode version trong tài liệu |
| Đường dẫn dist frontend | `frontendDist: "../dist"`, `devUrl: "http://localhost:1420"` | Brief §5 |
| Vitest config | Tách riêng `vitest.config.ts` | Giữ `vite.config.ts` tối giản cho Tauri; config test (jsdom, include `tests/`) không leak vào build |
| SQLx query style | **Chỉ dùng runtime query** (`sqlx::query`), không dùng macro `query!` | Tránh phức tạp `DATABASE_URL`/offline `.sqlx` cache; Phase 1 chỉ có một migration và không có query nào đáng để compile-check |
| Phương pháp scaffold | **Viết tay toàn bộ** (không dùng `npm create vite` / `npx tauri init`) | User đã confirm. Scaffolder tương tác có nguy cơ ghi đè `AGENTS.md`; viết file trực tiếp rồi `npm install` |
| Dependency tokio | Không khai báo trực tiếp trong runtime deps | Tauri đã cung cấp async runtime (brief §3.4); feature `runtime-tokio` của sqlx là đủ. Có thể thêm `tokio` vào **dev-dependencies** nếu cần macro `#[tokio::test]` cho Rust DB test |
| Node version | Dùng v24.18.0 đã cài; `engines.node >=20.19` làm sàn | Brief khuyến nghị 20.19+/22.12+ LTS nhưng v24 vẫn chạy tốt với Vite 6/7 + React 19. Ghi version thực tế trong README |

---

## Các bước triển khai

### Bước 1 — Baseline repo & kiểm tra toolchain (Task 1 của brief)

**File cần tạo:**
- `D:\Project\Mangav2\.gitignore`
- `D:\Project\Mangav2\.editorconfig`
- `D:\Project\Mangav2\README.md` (stub; hoàn thiện ở Bước 14)

**Hành động:**
1. Kiểm tra toolchain: `node --version`, `npm --version`, `rustc --version`, `cargo --version`, `rustup show` (xác nhận target `x86_64-pc-windows-msvc`). Kiểm tra VS Build Tools C++ workload đã cài chưa (nếu thiếu, `cargo check` sau này sẽ fail). Kiểm tra WebView2 runtime: probe registry `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` hoặc defer đến lúc test `tauri dev` thủ công.
2. Tạo `.gitignore`:
   ```
   node_modules/
   dist/
   src-tauri/target/
   src-tauri/gen/
   *.db
   *.db-shm
   *.db-wal
   *.sqlite
   *.sqlite3
   .env
   .env.local
   ```
   **KHÔNG ignore** `package-lock.json`, `Cargo.lock`.
3. Tạo `.editorconfig` (indent 2 space, UTF-8, line ending LF, final newline).
4. Tạo `README.md` stub (tiêu đề + "Phase 01 scaffold in progress").
5. `git init` trong `D:\Project\Mangav2`.
6. `git add AGENTS.md .gitignore .editorconfig README.md`
7. `git commit -m "chore: baseline — preserve AGENTS.md, add gitignore/editorconfig"`

**Nghiệm thu:** `git log --oneline` hiện commit baseline; `git status` clean; `AGENTS.md` không bị thay đổi (so sánh hash trước/sau).

---

### Bước 2 — Scaffold config frontend (Task 2 của brief, phần 1)

**File cần tạo:**
- `D:\Project\Mangav2\package.json`
- `D:\Project\Mangav2\tsconfig.json`
- `D:\Project\Mangav2\tsconfig.app.json`
- `D:\Project\Mangav2\tsconfig.node.json`
- `D:\Project\Mangav2\vite.config.ts`
- `D:\Project\Mangav2\index.html`
- `D:\Project\Mangav2\src\main.tsx`
- `D:\Project\Mangav2\src\vite-env.d.ts`
- `D:\Project\Mangav2\src\app\App.tsx` (placeholder: `<div>Manga Downloader 1989</div>`)

**Hành động:**
1. Tạo `package.json`:
   - `name`: `"manga-downloader-1989"`
   - `private`: `true`
   - `type`: `"module"`
   - `engines.node`: `">=20.19"`
   - `scripts`: theo brief §9 (`dev`, `build`, `preview`, `lint`, `typecheck`, `test`, `test:watch`, `tauri`)
   - `dependencies`: `react`, `react-dom`, `@tauri-apps/api`
   - `devDependencies`: `typescript`, `vite`, `@vitejs/plugin-react`, `@tauri-apps/cli`, `eslint`, `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `vitest`, `jsdom`, `@testing-library/react`, `@types/react`, `@types/react-dom`
   - **Không hardcode version** — dùng `"latest"` hoặc bỏ trống, để `npm install` tự resolve. Sau đó khóa qua `package-lock.json`.
2. Tạo `tsconfig.json` (solution-style, references `tsconfig.app.json` + `tsconfig.node.json`).
3. Tạo `tsconfig.app.json` (target ES2020, lib DOM+ES2020, jsx react-jsx, module ESNext, moduleResolution bundler, strict, paths `@/*` → `./src/*` optional).
4. Tạo `tsconfig.node.json` (cho vite.config.ts: target ES2022, lib ES2023, module ESNext, composite true).
5. Tạo `vite.config.ts`:
   ```ts
   import { defineConfig } from "vite";
   import react from "@vitejs/plugin-react";

   export default defineConfig({
     plugins: [react()],
     clearScreen: false,
     server: {
       port: 1420,
       strictPort: true,
     },
     envPrefix: ["VITE_", "TAURI_"],
     build: {
       target: "es2021",
       outDir: "dist",
     },
   });
   ```
6. Tạo `index.html` (title `Manga Downloader 1989`, mount `#root`, load `/src/main.tsx`).
7. Tạo `src/main.tsx` (render `<App />` từ `./app/App`).
8. Tạo `src/vite-env.d.ts` (`/// <reference types="vite/client" />`).
9. Tạo `src/app/App.tsx` placeholder (div đơn giản với tên app; sẽ thay ở Bước 6).
10. Chạy `npm install` trong `D:\Project\Mangav2`.

**Nghiệm thu:** `npm install` exit 0; `npx vite build` thành công (tạo `dist/`); `package-lock.json` được sinh ra.

---

### Bước 3 — Scaffold Tauri/Rust (Task 2 của brief, phần 2)

**File cần tạo:**
- `D:\Project\Mangav2\src-tauri\Cargo.toml`
- `D:\Project\Mangav2\src-tauri\build.rs`
- `D:\Project\Mangav2\src-tauri\tauri.conf.json`
- `D:\Project\Mangav2\src-tauri\src\main.rs`
- `D:\Project\Mangav2\src-tauri\src\lib.rs` (placeholder tối giản)

**Hành động:**
1. Tạo `src-tauri/Cargo.toml`:
   ```toml
   [package]
   name = "manga-downloader-1989"
   version = "0.1.0"
   edition = "2021"

   [lib]
   name = "manga_downloader_1989_lib"
   crate-type = ["staticlib", "cdylib", "rlib"]

   [build-dependencies]
   tauri-build = { version = "2", features = [] }

   [dependencies]
   tauri = { version = "2", features = [] }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "migrate", "macros"] }
   thiserror = "2"

   [dev-dependencies]
   tempfile = "3"
   # tokio = { version = "1", features = ["macros", "rt"] }  # bỏ comment nếu cần #[tokio::test]
   ```
2. Tạo `src-tauri/build.rs`:
   ```rust
   fn main() {
       tauri_build::build()
   }
   ```
3. Tạo `src-tauri/tauri.conf.json` (schema Tauri 2):
   - `productName`: `"Manga Downloader 1989"`
   - `version`: `"0.1.0"`
   - `identifier`: `"com.manga1989.downloader"`
   - `build.devUrl`: `"http://localhost:1420"`
   - `build.frontendDist`: `"../dist"`
   - `build.beforeDevCommand`: `"npm run dev"`
   - `build.beforeBuildCommand`: `"npm run build"`
   - `app.withGlobalTauri`: `false`
   - `app.security.csp`: (set ở Bước 9; placeholder `"default-src 'self'"` tạm)
   - `app.windows[0]`: `{ "label": "main", "title": "Manga Downloader 1989", "width": 1100, "height": 720, "minWidth": 800, "minHeight": 600 }`
   - `bundle.active`: `true`
   - `bundle.targets`: `["msi", "nsis"]` hoặc `"all"`
   - Target Windows x64
4. Tạo `src-tauri/src/main.rs`:
   ```rust
   #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

   fn main() {
       manga_downloader_1989_lib::run()
   }
   ```
5. Tạo `src-tauri/src/lib.rs` (placeholder tối giản):
   ```rust
   pub fn run() {
       tauri::Builder::default()
           .run(tauri::generate_context!())
           .expect("error while running tauri application");
   }
   ```
6. Chạy `cargo check` trong `D:\Project\Mangav2\src-tauri`.

**Nghiệm thu:** `cargo check` exit 0 (sinh ra `Cargo.lock`); không có lỗi compile.

---

### Bước 4 — Design tokens & global styles (Task 3 của brief, phần 1)

**File cần tạo:**
- `D:\Project\Mangav2\src\styles\tokens.css`
- `D:\Project\Mangav2\src\styles\globals.css`

**Hành động:**
1. Tạo `src/styles/tokens.css` với Dark Purple tokens từ brief §8 Task 3:
   ```css
   :root {
     --color-background: #101016;
     --color-sidebar: #171720;
     --color-surface: #20202C;
     --color-elevated: #292938;
     --color-primary: #A78BFA;
     --color-primary-hover: #8B5CF6;
     --color-text: #F5F3FF;
     --color-text-muted: #A1A1B5;
   }
   ```
2. Tạo `src/styles/globals.css`:
   - Import `./tokens.css`
   - Base reset (box-sizing, margin/padding)
   - `body`: background `var(--color-background)`, color `var(--color-text)`, font-family system-ui/sans-serif
   - Dark-first, đọc được ở kích thước cửa sổ nhỏ
   - Không hardcode màu — chỉ dùng CSS variables

**Nghiệm thu:** File tồn tại; `npm run typecheck` vẫn pass (CSS không ảnh hưởng TS).

---

### Bước 5 — IPC contracts & wrapper (Task 3 của brief, phần 2 + phần frontend của Task 5)

**File cần tạo:**
- `D:\Project\Mangav2\src\shared\api\contracts.ts`
- `D:\Project\Mangav2\src\shared\api\desktop.ts`

**Hành động:**
1. Tạo `src/shared/api/contracts.ts` với đúng types từ brief §6:
   ```ts
   export type AppInfo = {
     name: string;
     version: string;
     platform: string;
   };

   export type AppHealth = {
     status: "ok";
     database: "ready";
   };

   export type AppError = {
     code: "DATABASE_INIT_FAILED" | "INTERNAL_ERROR";
     message: string;
     retryable: boolean;
   };
   ```
2. Tạo `src/shared/api/desktop.ts`:
   - **File duy nhất** import `invoke` từ `@tauri-apps/api/core`
   - Export `getAppInfo(): Promise<AppInfo>` → gọi `invoke("app_get_info")`
   - Export `getAppHealth(): Promise<AppHealth>` → gọi `invoke("app_health_check")`
   - Export helper `normalizeIpcError(unknown): AppError` (không bao giờ `any`; trích `code`/`message`/`retryable` từ Tauri invoke rejection, fallback `INTERNAL_ERROR`)
   - Component không bao giờ gọi `invoke` trực tiếp

**Nghiệm thu:** `npm run typecheck` pass; `npm run lint` pass (không có unused import).

---

### Bước 6 — Shell UI (Task 3 của brief, phần 3)

**File cần tạo/sửa:**
- `D:\Project\Mangav2\src\app\App.tsx` (thay placeholder)
- `D:\Project\Mangav2\src\features\system\SystemStatus.tsx`

**Hành động:**
1. Thay `src/app/App.tsx`:
   - Import `./styles/globals.css` (hoặc đảm bảo `main.tsx` import nó)
   - Render tiêu đề app "Manga Downloader 1989"
   - Render `<SystemStatus />`
   - **Không có nav/button/link giả** cho tính năng chưa triển khai
2. Tạo `src/features/system/SystemStatus.tsx`:
   - On mount: gọi `getAppInfo()` và `getAppHealth()` từ `@/shared/api/desktop`
   - State phân biệt: loading (spinner/text), error (hiện `AppError.message` đã normalize), success (hiện name, version, platform, DB status)
   - Handle rejected promise với `normalizeIpcError`
   - Dùng CSS variables cho màu (không hardcode hex)

**Nghiệm thu:** `npm run typecheck` pass; `npm run lint` pass. Kiểm tra runtime đầy đủ cần Bước 8 (backend commands).

---

### Bước 7 — Bootstrap DB & migration (Task 4 của brief)

**File cần tạo:**
- `D:\Project\Mangav2\src-tauri\migrations\0001_init.sql`
- `D:\Project\Mangav2\src-tauri\src\error.rs`
- `D:\Project\Mangav2\src-tauri\src\db\mod.rs`
- `D:\Project\Mangav2\src-tauri\src\app_state.rs`

**File cần sửa:**
- `D:\Project\Mangav2\src-tauri\src\lib.rs` (thêm setup hook)

**Hành động:**
1. Tạo `src-tauri/migrations/0001_init.sql` (đúng DDL từ brief §7):
   ```sql
   CREATE TABLE app_settings (
       key TEXT PRIMARY KEY NOT NULL,
       value_json TEXT NOT NULL,
       updated_at TEXT NOT NULL
   );
   ```
   **Không seed data. Không bảng khác.**
2. Tạo `src-tauri/src/error.rs`:
   - Enum `AppError` qua `thiserror`: variants `DatabaseInit`, `Internal`
   - Impl `serde::Serialize` thủ công để ra wire shape `{ code: "DATABASE_INIT_FAILED"|"INTERNAL_ERROR", message, retryable }`
   - Message đã sanitize (không path/SQL/stack trace)
   - `impl From<AppError> for tauri::InvokeError` (hoặc trả `Result<T, AppError>` với bound `Serialize` — pattern chuẩn Tauri 2)
3. Tạo `src-tauri/src/db/mod.rs`:
   - `pub async fn init_pool(app_data_dir: &Path) -> Result<SqlitePool, AppError>`
   - `create_dir_all(app_data_dir)`
   - `SqliteConnectOptions` với `create_if_missing(true)`, `foreign_keys(true)`
   - `SqlitePoolOptions` (max_connections ~5)
   - `sqlx::migrate!("./migrations").run(&pool).await` — macro resolve relative với `CARGO_MANIFEST_DIR` (= `src-tauri/`), nên `./migrations` hoạt động
   - **Không config WAL** (brief §7: không thêm storage config khó đảo ngược)
4. Tạo `src-tauri/src/app_state.rs`:
   ```rust
   pub struct AppState {
       pub db: sqlx::SqlitePool,
   }
   ```
5. Sửa `src-tauri/src/lib.rs`:
   - Trong `tauri::Builder::default()`, thêm `.setup(|app| { ... })` hook
   - Trong setup: dùng `tauri::async_runtime::block_on` (hoặc async setup nếu Tauri 2 hỗ trợ) để:
     - Resolve `app.path().app_data_dir()`
     - Gọi `db::init_pool(&app_data_dir).await`
     - Nếu thành công: `.manage(AppState { db })`
     - Nếu fail: log diagnostic (chỉ dev) và **exit startup với lỗi** — không có chế độ fake-healthy
   - Thêm `mod app_state; mod db; mod error;`

**Nghiệm thu:** `cargo check` pass; việc tạo DB sẽ verify ở Bước 13 khi chạy `tauri dev` thủ công (lần đầu tạo `%APPDATA%\com.manga1989.downloader\*.db`, lần sau mở lại).

---

### Bước 8 — System commands & đăng ký handler (Task 5 của brief, phần backend)

**File cần tạo:**
- `D:\Project\Mangav2\src-tauri\src\commands\mod.rs`
- `D:\Project\Mangav2\src-tauri\src\commands\system.rs`

**File cần sửa:**
- `D:\Project\Mangav2\src-tauri\src\lib.rs` (đăng ký handler)

**Hành động:**
1. Tạo `src-tauri/src/commands/mod.rs`:
   ```rust
   pub mod system;
   ```
2. Tạo `src-tauri/src/commands/system.rs`:
   - Struct với `#[derive(Serialize)] #[serde(rename_all = "camelCase")]`:
     - `AppInfo { name: String, version: String, platform: String }`
     - `AppHealth { status: String, database: String }` (literal `"ok"` / `"ready"`)
   - `#[tauri::command] async fn app_get_info(app: tauri::AppHandle) -> Result<AppInfo, AppError>`:
     - Lấy name/version từ `app.package_info()` (hoặc config `tauri::AppHandle`)
     - Lấy platform từ `std::env::consts::OS` hoặc Tauri platform string
   - `#[tauri::command] async fn app_health_check(state: tauri::State<'_, AppState>) -> Result<AppHealth, AppError>`:
     - Optional chạy `sqlx::query("SELECT 1").execute(&state.db).await` để chứng minh pool sống
     - Trả `AppHealth { status: "ok".into(), database: "ready".into() }`
3. Sửa `src-tauri/src/lib.rs`:
   - Thêm `mod commands;`
   - Trong `tauri::Builder::default()`, thêm:
     ```rust
     .invoke_handler(tauri::generate_handler![
         commands::system::app_get_info,
         commands::system::app_health_check
     ])
     ```
   - **Đúng hai command.** Không thêm.

**Nghiệm thu:** `cargo check` pass; `cargo clippy` clean.

---

### Bước 9 — Hardening bảo mật (Task 6 của brief)

**File cần tạo:**
- `D:\Project\Mangav2\src-tauri\capabilities\default.json`

**File cần sửa:**
- `D:\Project\Mangav2\src-tauri\tauri.conf.json` (CSP)
- `D:\Project\Mangav2\src-tauri\build.rs` (optional: `AppManifest` command allowlist)

**Hành động:**
1. Tạo `src-tauri/capabilities/default.json` (format Tauri 2, **không phải** allowlist v1):
   ```json
   {
     "$schema": "../gen/schemas/desktop-schema.json",
     "identifier": "default",
     "windows": ["main"],
     "permissions": ["core:default"]
   }
   ```
   - **Không** `shell:default`, `fs:default`, `http:default`, không plugin, không remote URL
   - `core:default` gồm quyền core tối thiểu (event, path, app, v.v.)
   - Nếu cần chặt hơn, liệt kê chỉ những core permission cần — ghi chú lựa chọn trong README
2. Sửa `src-tauri/tauri.conf.json` → `app.security.csp`:
   ```
   default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost
   ```
   - Tauri tự inject nonce/hash cho script của nó lúc runtime
   - **Dev/HMR:** Tauri dev server serve qua Vite `devUrl` tại `http://localhost:1420`; Tauri tự mở rộng CSP cho dev. Nếu Vite HMR WebSocket bị chặn, thêm nới lỏng chỉ-dev (Tauri v2 tự xử origin `devUrl`) — **không bao giờ thêm `*`**
   - Verify trong devtools console lúc chạy thủ công; KHÔNG nới production CSP để chữa lỗi dev
3. **Optional** trong `src-tauri/build.rs`:
   ```rust
   fn main() {
       tauri_build::Attributes::new()
           .app_manifest(tauri_build::AppManifest::new().commands(&[
               "app_get_info",
               "app_health_check",
           ]))
           .build()
   }
   ```
   - Confirm hỗ trợ trên version Tauri đã khóa; bỏ qua nếu lỗi và ghi chú trong handoff

**Nghiệm thu:** `cargo check` pass; khi chạy `npm run tauri dev` thủ công, devtools không báo CSP violation; chỉ window `main` có capability.

---

### Bước 10 — Setup test frontend & smoke test (Task 7 của brief, phần 1)

**File cần tạo:**
- `D:\Project\Mangav2\eslint.config.js`
- `D:\Project\Mangav2\vitest.config.ts`
- `D:\Project\Mangav2\tests\app.test.tsx`
- `D:\Project\Mangav2\tests\setup.ts` (optional)

**Hành động:**
1. Tạo `eslint.config.js` (flat config):
   - `typescript-eslint` recommended
   - `eslint-plugin-react-hooks`
   - `eslint-plugin-react-refresh`
   - Không rule thừa
2. Tạo `vitest.config.ts`:
   ```ts
   import { defineConfig } from "vitest/config";
   import react from "@vitejs/plugin-react";

   export default defineConfig({
     plugins: [react()],
     test: {
       environment: "jsdom",
       include: ["tests/**/*.test.tsx"],
       setupFiles: ["./tests/setup.ts"],
     },
   });
   ```
3. Tạo `tests/setup.ts` (optional):
   - Mock `@tauri-apps/api/core` global nếu cần, hoặc per-test
4. Tạo `tests/app.test.tsx`:
   - `vi.mock("@tauri-apps/api/core")` ở module level
   - Mock `invoke("app_get_info")` → resolve với fixture `{ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" }`
   - Mock `invoke("app_health_check")` → resolve với `{ status: "ok", database: "ready" }`
   - Render `<App />` (hoặc `<SystemStatus />`)
   - Assert loading state hiện ra, sau đó name/version/`ready` hiển thị
   - Một test mà `invoke` reject với `{ code: "INTERNAL_ERROR", message: "Test error", retryable: false }` → assert error state render
   - **Không network, không DB thật**

**Nghiệm thu:** `npm run lint` exit 0; `npm run test` exit 0; không có file DB trong repo sau khi test.

---

### Bước 11 — Rust DB test (Task 7 của brief, phần 2)

**File cần sửa:**
- `D:\Project\Mangav2\src-tauri\src\db\mod.rs` (thêm `#[cfg(test)] mod tests`)
- `D:\Project\Mangav2\src-tauri\Cargo.toml` (bỏ comment `tokio` dev-dep nếu cần)

**Hành động:**
1. Trong `src-tauri/src/db/mod.rs`, thêm module test:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use tempfile::tempdir;

       #[tokio::test]
       async fn test_init_pool_creates_app_settings() {
           let dir = tempdir().unwrap();
           let pool = init_pool(dir.path()).await.unwrap();
           let row: (String,) = sqlx::query_as(
               "SELECT name FROM sqlite_master WHERE type='table' AND name='app_settings'"
           )
           .fetch_one(&pool)
           .await
           .unwrap();
           assert_eq!(row.0, "app_settings");
       }

       #[tokio::test]
       async fn test_init_pool_idempotent() {
           let dir = tempdir().unwrap();
           let _pool1 = init_pool(dir.path()).await.unwrap();
           let _pool2 = init_pool(dir.path()).await.unwrap(); // mở lại cùng dir
           // Nếu migration idempotent, sẽ không panic
       }
   }
   ```
2. Nếu macro `#[tokio::test]` không truy cập được mà thiếu tokio dev-dep trực tiếp, bỏ comment trong `Cargo.toml`:
   ```toml
   [dev-dependencies]
   tempfile = "3"
   tokio = { version = "1", features = ["macros", "rt"] }
   ```
   Điều này được phép vì code test thực sự gọi tokio API (brief §3.4).

**Nghiệm thu:** `cargo test` trong `src-tauri/` exit 0; không để lại file DB trong repo (tempdir tự dọn).

---

### Bước 12 — Quality gates headless (Task 7 của brief, phần 3)

**Hành động:**
Chạy theo thứ tự từ `D:\Project\Mangav2`:
1. `npm ci` (clean install từ lockfile)
2. `npm run lint`
3. `npm run typecheck`
4. `npm run test`
5. `npm run build`
6. `cd src-tauri && cargo check && cargo clippy && cargo test && cd ..`

**Fix mọi failure.** Lỗi thường gặp:
- Lint: unused import, thiếu deps trong React hooks
- Typecheck: type lệch giữa contracts và desktop.ts
- Test: mock setup sai, không tìm thấy component
- Build: lỗi config Vite, thiếu dependency
- Cargo check: path `sqlx::migrate!` sai, thiếu feature flag
- Cargo clippy: clone không cần thiết, closure thừa
- Cargo test: tokio runtime không khả dụng, lỗi path tempdir

**Nghiệm thu:** Mọi lệnh exit 0. Ghi lại output thực tế cho handoff.

---

### Bước 13 — Verify desktop thủ công (Task 7 của brief, phần 4)

**Hành động:**
1. `npm run tauri dev` — mở cửa sổ desktop
   - **Check:** Tiêu đề cửa sổ "Manga Downloader 1989"
   - **Check:** Nền dark purple (`#101016`)
   - **Check:** Shell hiện tên app, version (`0.1.0`), platform (`windows`), DB status (`ready`)
   - **Check:** Không có CSP violation trong devtools console (F12 → Console)
   - **Check:** File DB được tạo tại `%APPDATA%\com.manga1989.downloader\` (verify path trong log Rust hoặc qua debug print `app.path().app_data_dir()` — **không commit path máy cụ thể**)
   - **Check:** Đóng app, mở lại — DB mở lại thành công (migration idempotent)
2. **Optional:** `npm run tauri build` — sinh ra installer/exe Windows
   - Chậm (5-15 phút); yêu cầu MSVC + WebView2
   - Output: `src-tauri/target/release/bundle/msi/*.msi` và/hoặc `nsis/*.exe`
   - Nếu agent không chạy được (không có Windows desktop session), đánh dấu `not run` kèm lý do

**Nghiệm thu:** Các check thủ công pass; ghi lại kết quả trung thực (pass/fail/not-run).

---

### Bước 14 — README & handoff (Task 8 của brief)

**File cần sửa:**
- `D:\Project\Mangav2\README.md` (viết lại hoàn toàn stub)

**Hành động:**
1. Viết lại `README.md` với:
   - **Stack:** React 19 + TypeScript + Vite 6/7 + Tauri 2 + Rust 1.98 + SQLx 0.8 + SQLite
   - **Prerequisite Windows:**
     - Node.js (ghi version thực tế đã dùng, vd v24.18.0; chú thích brief khuyến nghị 20.19+/22.12+ LTS)
     - npm (đi kèm Node)
     - Rust stable MSVC (`rustup default stable-msvc`)
     - Visual Studio Build Tools với workload "Desktop development with C++" + Windows SDK
     - Microsoft Edge WebView2 Runtime (phần lớn Win10/11 đã có; nếu thiếu, cài Evergreen Runtime)
     - Tauri CLI v2 (cài local làm devDependency, không cần cài global)
   - **Cài & chạy:**
     ```powershell
     npm ci
     npm run tauri dev
     ```
   - **Quality gates:**
     ```powershell
     npm run lint
     npm run typecheck
     npm run test
     npm run build
     cd src-tauri; cargo check; cargo clippy; cargo test; cd ..
     npm run tauri build  # chậm, sinh ra installer
     ```
   - **Cấu trúc project:** tóm tắt ngắn `src/app`, `src/features`, `src/shared`, `src/styles`, `src-tauri/src`, `src-tauri/migrations`, `src-tauri/capabilities`
   - **Vị trí DB:** `%APPDATA%\com.manga1989.downloader\` (Windows), qua API Tauri `app.path().app_data_dir()`. **Không commit path máy cụ thể.**
   - **Giới hạn phạm vi Phase 1:** Ghi rõ CHƯA triển khai: import URL, tìm kiếm trong app, hàng đợi tải, thư viện, trình đọc offline, bảng manga/chapter/source/download trong DB. Đó là Phase 2+.
   - **Ghi chú handoff:** theo format brief §12
2. `git add .` (mọi file mới + lockfiles)
3. `git status` — verify `AGENTS.md` không bị thay đổi (so sánh hash với commit baseline)
4. `git commit -m "feat: Phase 01 scaffold — Tauri 2 + React + SQLx foundation"`

**Nghiệm thu:** `git log --oneline` hiện hai commit (baseline + scaffold); `git diff HEAD~1 AGENTS.md` trống; README chính xác.

---

## Critical Path & Song song hóa

**Critical path:** Bước 1 → 2 → 3 → 7 → 8 → 9 → 12 → 13 → 14

**Có thể song song:**
- Bước 4 (tokens) + Bước 5 (contracts) chạy song song với Bước 3 (Rust scaffold) — chỉ cần tsconfig/vite của Bước 2 để typecheck
- Bước 6 (shell UI) phụ thuộc Bước 5 nhưng có thể viết trước khi Bước 8 (backend commands) tồn tại — verify runtime gộp vào Bước 13
- Bước 10 (frontend test) có thể viết ngay khi Bước 6 xong (mock bỏ dependency backend)
- Bước 11 (Rust test) cần Bước 7
- ESLint config (Bước 10) có thể viết bất cứ lúc nào sau Bước 2

**Nút thắt:**
- `npm install` (Bước 2) — network-bound, ~2-5 phút
- `cargo check` (Bước 3, 7, 8) — compile Tauri + SQLx từ đầu lần đầu, ~5-15 phút
- `npm run tauri build` (Bước 13, optional) — build release đầy đủ, ~10-20 phút

---

## Rủi ro & Giảm thiểu

| Rủi ro | Khả năng | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| **Node v24 vs brief 20.19+/22.12+** | Thấp | Thấp | Vite 6/7 + React 19 hỗ trợ Node 24. Ghi version thực tế trong README; `engines.node >=20.19` làm sàn |
| **Thiếu MSVC/WebView2** | Trung bình | Cao | Verify ở Bước 1 (`rustup show`, probe registry) trước khi scaffold nặng. `cargo check` fail nếu thiếu C++ Build Tools; `tauri dev` fail nếu thiếu WebView2. Ghi cách fix trong README |
| **Macro compile-time SQLx** | N/A | N/A | Cố ý tránh (`query!` cần `DATABASE_URL` hoặc offline cache). Mọi query là runtime `sqlx::query`. Nếu dev sau thêm `query!`, đó là quyết định Phase 2 |
| **Resolve path `sqlx::migrate!`** | Trung bình | Cao | Macro resolve relative với `CARGO_MANIFEST_DIR` (= `src-tauri/`), nên `sqlx::migrate!("./migrations")` yêu cầu `src-tauri/migrations/` tồn tại **trước khi** `cargo check`. Tạo file migration ở Bước 7 trước khi compile |
| **CSP vs Vite HMR** | Trung bình | Trung bình | CSP production set chặt; lỗi HMR dev được giải quyết bằng CSP injection chế độ dev của Tauri — không bao giờ wildcard. Nếu có violation, fix resource chứ không fix policy. Verify qua devtools lúc chạy thủ công |
| **Nhầm format capability (v1 vs v2)** | Trung bình | Cao | Phải là file JSON capability với mảng `windows` + `permissions`, được reference tự động từ `src-tauri/capabilities/`. Style v1 `tauri.conf.json > allowlist` sẽ fail schema validation trên Tauri 2 |
| **Vitest jsdom + Tauri** | Cao | Thấp | `invoke` throw bên ngoài Tauri webview. Mọi test render shell component phải `vi.mock("@tauri-apps/api/core")` ở module level. Thêm ghi chú lint-level hoặc guard trong `tests/setup.ts` nếu cần |
| **Khả dụng `#[tokio::test]`** | Trung bình | Thấp | sqlx không re-export gì cho macro test. Có thể cần `tokio = { version = "1", features = ["macros", "rt"] }` làm **dev-dependency** — được phép vì code test thực sự dùng (brief §3.4 cho phép khai báo tokio khi code thực sự gọi). Ghi chú trong handoff |
| **Path app data dir** | Thấp | Thấp | Pattern đã confirm trên Windows = `%APPDATA%\<identifier>\` tức `C:\Users\<user>\AppData\Roaming\com.manga1989.downloader\`. Verify lúc runtime khi chạy `tauri dev` thủ công (log path chỉ trong bản dev, không đưa ra UI) |
| **Scaffolder ghi đè AGENTS.md** | N/A | N/A | User đã chọn viết tay — không bao giờ chạy `npm create vite` / `npx tauri init` tại chỗ. Integrity của `AGENTS.md` được check qua git ở mọi bước |
| **Thay đổi API Tauri 2** | Trung bình | Trung bình | Brief tham khảo docs Tauri 2 nhưng API có thể đã dịch chuyển trong bản 2.x mới nhất. Nếu `app.path().app_data_dir()` hoặc cú pháp `generate_handler!` khác, tham khảo docs của version Tauri đã khóa và adapt. Ghi chú deviation trong handoff |
| **SQLx 0.8 vs 0.7** | Thấp | Trung bình | Brief nói "SQLx 0.8.x" nhưng 0.8 có thể chưa stable. Nếu `cargo install sqlx --version 0.8` fail, dùng 0.7 (stable mới nhất). Khóa qua `Cargo.lock` và ghi chú trong README |

---

## Chiến lược Verification

### Headless (agent có thể chạy tự động)

| Lệnh | Mục đích | Kỳ vọng |
|---|---|---|
| `npm ci` | Clean install từ lockfile | Exit 0 |
| `npm run lint` | ESLint flat config | Exit 0, không lỗi |
| `npm run typecheck` | `tsc -b --pretty false` | Exit 0, không lỗi type |
| `npm run test` | `vitest run` | Exit 0, mọi test pass |
| `npm run build` | `tsc -b && vite build` | Exit 0, sinh ra `dist/` |
| `cd src-tauri && cargo check` | Check compile Rust | Exit 0, sinh ra `Cargo.lock` |
| `cd src-tauri && cargo clippy` | Linter Rust | Exit 0, không warning (hoặc chỉ pedantic) |
| `cd src-tauri && cargo test` | Unit test Rust | Exit 0, DB test pass |
| `git status` | Verify state clean | Không có file untracked trừ `dist/`, `target/`, `*.db` (đều đã gitignore) |
| `git diff HEAD~1 AGENTS.md` | Verify AGENTS.md không đổi | Diff trống |

### Phiên desktop Windows thủ công (user chạy, hoặc agent chạy và báo cáo trung thực)

| Lệnh | Mục đích | Kỳ vọng |
|---|---|---|
| `npm run tauri dev` | Mở cửa sổ desktop | Tiêu đề "Manga Downloader 1989", nền dark purple, shell hiện name/version/platform/DB-ready, không lỗi CSP trong devtools, DB tạo tại `%APPDATA%\com.manga1989.downloader\` |
| Đóng + mở lại app | Idempotency DB | Lần chạy thứ hai mở lại DB thành công, migration không chạy lại (check bảng `_sqlx_migrations`) |
| `npm run tauri build` | Sinh installer/exe Windows | Chậm (10-20 phút); output `src-tauri/target/release/bundle/msi/*.msi` và/hoặc `nsis/*.exe`. Nếu agent không chạy được (không có Windows desktop session), đánh dấu `not run` kèm lý do |

**Theo brief §9:** Bất kỳ gate nào không chạy được phải báo cáo `not run` kèm lý do — không bao giờ đánh dấu pass.

---

## Đề xuất TaskCreate Breakdown

Dùng `TaskCreate` để track 14 task rời rạc này. Dependencies ghi dạng `(deps: X, Y)`.

1. **Baseline repo & kiểm tra toolchain** — verify Node/Rust MSVC/WebView2, tạo `.gitignore`/`.editorconfig`, `git init`, commit baseline `AGENTS.md`. (deps: không có)
2. **Scaffold config frontend** — `package.json`, tsconfigs, `vite.config.ts`, `index.html`, `src/main.tsx`, `src/vite-env.d.ts`, placeholder `App.tsx`, `npm install`. (deps: 1)
3. **Scaffold Tauri/Rust** — `src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json`, `main.rs`, placeholder `lib.rs`, `cargo check`. (deps: 2)
4. **Design tokens & global styles** — `src/styles/tokens.css`, `globals.css` với 8 token Dark Purple. (deps: 2)
5. **IPC contracts & wrapper** — `contracts.ts`, `desktop.ts` với chuẩn hóa lỗi. (deps: 2)
6. **Shell UI** — `App.tsx`, `SystemStatus.tsx` với state loading/error/success. (deps: 4, 5)
7. **Bootstrap DB & migration** — `0001_init.sql`, `db/mod.rs`, `error.rs`, `app_state.rs`, wire vào setup `lib.rs`. (deps: 3)
8. **System commands & đăng ký handler** — `commands/mod.rs`, `commands/system.rs`, đăng ký hai command trong `lib.rs`. (deps: 7)
9. **Hardening bảo mật** — `capabilities/default.json`, CSP trong `tauri.conf.json`, optional `AppManifest` command allowlist. (deps: 8)
10. **Setup test frontend & smoke test** — `eslint.config.js`, `vitest.config.ts`, `tests/app.test.tsx` với mock invoke. (deps: 6)
11. **Rust DB test** — test init/migration/idempotency基于 tempfile (thêm tokio dev-dep nếu cần). (deps: 7)
12. **Quality gates headless** — chạy lint/typecheck/test/build/cargo check/clippy/test, fix failures. (deps: 9, 10, 11)
13. **Verify desktop thủ công** — `npm run tauri dev` (cửa sổ, dữ liệu IPC sống, DB tại `%APPDATA%`, CSP clean), optional `npm run tauri build`, ghi pass/not-run trung thực. (deps: 12)
14. **README & handoff** — README đầy đủ (prereqs, lệnh, cấu trúc, vị trí DB, giới hạn phạm vi), báo cáo handoff theo brief §12, commit cuối. (deps: 13)

---

## Ngoài phạm vi rõ ràng (KHÔNG làm trong Phase 1)

- **Không code HTTP/crawler:** không `reqwest`, không `zip`, không dependency downloader
- **Không plugin Tauri:** filesystem, shell, http, dialog, notification, updater, autostart, deep-link, single-instance
- **Không thư viện UI:** Tailwind, shadcn/ui, Radix, Lucide, Motion, React Router, Zustand, TanStack Query, TanStack Virtual
- **Không bảng nghiệp vụ trong DB:** manga, chapter, source, download, library, user, session. Chỉ có `app_settings`
- **Không seed data:** migration tạo bảng `app_settings` trống, không có INSERT
- **Không config WAL:** không thêm storage config khó đảo ngược trong phase nền tảng
- **Không tạo thư mục feature trống:** `features/search`, `features/downloads`, `features/library`, `features/reader`, `features/settings` — chỉ tạo khi Phase 2+ cần
- **Không abstraction đầu cơ:** không build pattern repository/service generic "để dành cho tương lai"
- **Không UI giả:** button/nav/link ngụ ý tính năng chưa triển khai (vd "Import URL", "Search", "Downloads", "Library", "Settings")
- **Không ký installer:** không code signing certificate, không auto-update, không pipeline CI/CD
- **Không `withGlobalTauri`:** giữ Tauri API scope trong import `@tauri-apps/api`
- **Không remote origin trong capabilities:** không `https://*` hoặc `http://*` trong field `windows` hoặc `remote`
- **Không CSP wildcard:** không `*` trong `default-src`, `script-src`, `style-src`, `img-src`, v.v.
- **Không hardcode path DB:** không `./data`, không file cạnh exe, không commit path máy cụ thể vào README
- **Không sửa AGENTS.md:** bảo tồn nguyên trạng file workflow rules của user
- **Không Tokio runtime thứ hai:** chỉ dùng async runtime của Tauri
- **Không gọi `invoke()` trực tiếp:** mọi IPC đi qua wrapper `src/shared/api/desktop.ts`
- **Không commit build artifacts:** `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`, `*.db*` đều đã gitignore. Lockfiles (`package-lock.json`, `Cargo.lock`) ĐƯỢC commit

---

## File quan trọng cho Implementation

Những file dễ lỗi nhất và cần chú ý đặc biệt:

| File | Lý do quan trọng |
|---|---|
| `D:\Project\Mangav2\src-tauri\tauri.conf.json` | Identifier, CSP, devUrl/frontendDist, config window — schema validation fail nếu bất kỳ field nào sai |
| `D:\Project\Mangav2\src-tauri\src\lib.rs` | Setup hook: DB init, state management, đăng ký handler — async/await + lifecycle Tauri khó |
| `D:\Project\Mangav2\src-tauri\src\db\mod.rs` | Tạo pool, migration nhúng, idempotency, target test — resolve path `sqlx::migrate!` phải chính xác |
| `D:\Project\Mangav2\src\shared\api\desktop.ts` | Biên giới IPC duy nhất, chuẩn hóa lỗi — bất kỳ `invoke()` nào ngoài file này đều vi phạm kiến trúc |
| `D:\Project\Mangav2\src-tauri\capabilities\default.json` | Mô hình bảo mật Tauri 2, chỉ window main — format allowlist v1 sẽ fail |
| `D:\Project\Mangav2\src-tauri\migrations\0001_init.sql` | Đúng DDL từ brief §7 — không bảng thừa, không seed data |
| `D:\Project\Mangav2\package.json` | Scripts theo brief §9, deps không hardcode version, `engines.node` floor |
| `D:\Project\Mangav2\vite.config.ts` | Port 1420, strictPort, envPrefix, build target — Tauri dev server phụ thuộc vào đây |

---

## Format Báo cáo Handoff (theo brief §12)

Sau khi hoàn thành cả 14 task, tạo báo cáo handoff với:

1. **Tóm tắt thay đổi** theo khu vực: frontend, Rust/SQLite, bảo mật, tooling
2. **Danh sách file chính đã tạo/sửa** (path chính xác)
3. **Kết quả của từng lệnh quality gate** (lint, typecheck, test, build, cargo check/clippy/test, tauri dev, tauri build) — đánh dấu `pass`/`fail`/`not run` kèm lý do
4. **Vị trí DB runtime** theo API app data của Tauri (vd `%APPDATA%\com.manga1989.downloader\`) — KHÔNG commit path máy cụ thể
5. **Giả định hoặc vấn đề còn mở** cần quyết định trước Phase 2
6. **Xác nhận:** không có URL import, search, download, library, hoặc offline reader nào được triển khai trong Phase 1

**Checklist cho Reviewer (theo brief §12):**
- Scaffold có ghi đè file user nào đã tồn tại không? (verify hash `AGENTS.md` không đổi)
- Async DB init có block UI thread không? (phải dùng Tauri async runtime)
- Migration có được nhúng và idempotent theo SQLx history không? (check bảng `_sqlx_migrations`)
- Frontend có gọi command nào ngoài wrapper không? (grep `invoke(` bên ngoài `desktop.ts`)
- Capabilities/CSP có quá rộng không? (không wildcard, không remote origin, không plugin fs/shell/http)
- Có dependency không dùng không? (check `package.json` + `Cargo.toml` so với import thực tế)
- Test có dùng file DB user thật không? (chỉ được dùng tempfile/jsdom mock)
- README/handoff có tuyên bố quá mức không? (không "search works", "downloader ready", v.v.)

---

## Bước tiếp theo

1. **Thoát plan mode** (gọi `ExitPlanMode`)
2. **Tạo tasks** qua `TaskCreate` cho 14 mục trong "Đề xuất TaskCreate Breakdown"
3. **Thực thi Bước 1** (baseline repo & kiểm tra toolchain)
4. **Tiến hành tuần tự** qua Bước 2-14, cập nhật trạng thái task qua `TaskUpdate`
5. **Tạo báo cáo handoff** sau Bước 14

**Ước tính tổng thời gian:** 2-4 giờ (tùy thuộc thời gian compile `cargo check`, tốc độ mạng cho `npm install`, và có chạy `tauri build` hay không).
