# Manga Downloader 1989 — Phase 01 Implementation Brief

> **Dành cho:** Antigravity coding agent  
> **Phase:** 01 — Project Foundation & Architecture  
> **Target:** Windows desktop, x64 trước  
> **Trạng thái tài liệu:** Brief triển khai; chưa có nghĩa là ứng dụng đã được code.

## 1. Mục tiêu

Tạo nền tảng project có thể cài dependency, chạy desktop app trên Windows, gọi Rust command từ React, mở SQLite trong thư mục dữ liệu của ứng dụng và áp dụng migration khi khởi động. Mã nguồn phải có cấu trúc rõ ràng để các phase sau bổ sung:

- Nhập URL từ nguồn Manga được hỗ trợ.
- Tìm kiếm truyện trong ứng dụng.
- Quản lý hàng đợi tải chương.
- Thư viện truyện và dữ liệu local-first.
- Đọc chương offline.

Giao diện Phase 1 chỉ là một shell tối thiểu mang tên **Manga Downloader 1989**, dùng nền Dark Purple để xác nhận app đang chạy. Đây chưa phải bản thiết kế UI chi tiết hay một màn hình nghiệp vụ.

## 2. Trong phạm vi và ngoài phạm vi

### Trong phạm vi

- React + TypeScript + Vite cho frontend; Tauri 2 + Rust cho desktop/backend.
- Cấu hình build và dev cho Windows x64.
- Chia module frontend theo `app`, `features`, `shared`; chia Rust theo `commands`, `db`, `models`/`services` khi cần.
- Khởi tạo SQLite/SQLx và một migration nền tảng tối thiểu.
- Một lớp IPC frontend tập trung và hai command kiểm tra nền tảng.
- Dark Purple tokens tối thiểu cho shell.
- Script lint, type-check, test, build; README về cài môi trường và chạy project.
- Giới hạn quyền Tauri ở mức cần thiết cho shell; chưa cấp quyền filesystem/network cho webview.

### Ngoài phạm vi

- Cào dữ liệu, URL import, in-app search, adapter nguồn truyện.
- Tải ảnh/chương, retry, concurrency, progress events, đóng gói CBZ.
- Schema cho Manga/chapter/download/library; phase này chỉ tạo bảng settings tối thiểu.
- Trình đọc offline, quản lý thư viện, đồng bộ cloud, tài khoản.
- UI thương mại hoàn chỉnh, routing, state management, component library, animation.
- Installer phát hành, ký số, cập nhật tự động.

Không tạo nút hoặc luồng giả khiến người dùng nghĩ các tính năng ngoài phạm vi đã hoạt động.

## 3. Quyết định kiến trúc

1. **Một desktop app local-first:** React chỉ hiển thị giao diện và gọi typed IPC. Rust sở hữu truy cập filesystem, HTTP trong tương lai và dữ liệu SQLite. Frontend không mở trực tiếp database hay tự ghi đường dẫn tùy ý.
2. **Một ranh giới IPC:** mọi `invoke()` đi qua `src/shared/api/desktop.ts`. Tên command, payload và response dùng camelCase ở JSON; Rust dùng `serde(rename_all = "camelCase")` để khớp.
3. **SQLite qua SQLx trong Rust:** mở database dưới `app.path().app_data_dir()`; không lưu dữ liệu cạnh executable hoặc trong thư mục frontend. Pool được tạo một lần và quản lý bằng Tauri state. Migration nhúng trong binary, chạy lúc app khởi động.
4. **Tokio:** dùng async runtime do Tauri cung cấp cho async commands/SQLx. Không tạo Tokio runtime thứ hai. Chỉ khai báo dependency `tokio` trực tiếp khi mã ứng dụng thật sự gọi API của Tokio.
5. **Feature-based, nhưng chưa tạo feature rỗng:** phase sau thêm `features/search`, `features/downloads`, `features/library`, `features/reader`, `features/settings` theo nhu cầu. Không dựng sẵn nhiều abstraction hoặc thư mục trống.
6. **Không khóa phiên bản bằng số cứng trong tài liệu:** khi scaffold, chọn release tương thích mới nhất của Tauri 2, React, Vite, SQLx và khóa chính xác trong `package-lock.json` cùng `Cargo.lock`. Không nâng major tùy tiện sau khi đã scaffold.
7. **Lỗi IPC có cấu trúc:** backend trả lỗi đã được chuyển đổi sang kiểu serializable ổn định (`code`, `message`, `retryable`). Không gửi raw SQL, stack trace, token hoặc dữ liệu nhạy cảm về UI.

## 4. Cây thư mục đích sau Phase 1

```text
manga-downloader-1989/
├── README.md
├── .editorconfig
├── .gitignore
├── package.json
├── package-lock.json
├── tsconfig.json
├── tsconfig.app.json
├── tsconfig.node.json
├── vite.config.ts
├── index.html
├── eslint.config.js
├── src/
│   ├── main.tsx
│   ├── vite-env.d.ts
│   ├── app/
│   │   └── App.tsx
│   ├── features/
│   │   └── system/
│   │       └── SystemStatus.tsx
│   ├── shared/
│   │   └── api/
│   │       ├── contracts.ts
│   │       └── desktop.ts
│   └── styles/
│       ├── globals.css
│       └── tokens.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── migrations/
│   │   └── 0001_init.sql
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── app_state.rs
│       ├── error.rs
│       ├── commands/
│       │   ├── mod.rs
│       │   └── system.rs
│       └── db/
│           └── mod.rs
└── tests/
    └── app.test.tsx
```

Scaffold tool versions/templates-এর কারণে কিছু config filename khác nhau là chấp nhận được nếu tính năng tương đương và README phản ánh đúng. Giữ migration tại `src-tauri/migrations/` và không commit database runtime.

## 5. Dependencies và môi trường Windows

### Môi trường phát triển

- Windows 10/11 x64.
- Node.js theo yêu cầu Vite hiện hành (tài liệu hiện ghi Node.js `20.19+` hoặc `22.12+`); ưu tiên nhánh LTS được project khóa trong README.
- npm và `package-lock.json`; dùng cùng một package manager trong repo.
- Rust stable với target MSVC `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools với workload **Desktop development with C++** và Windows SDK.
- Microsoft Edge WebView2 Runtime; phần lớn Windows 10/11 đã có sẵn, nếu thiếu thì cài Evergreen Runtime.
- Tauri CLI v2.

### Dependency frontend

**Runtime:** `react`, `react-dom`, `@tauri-apps/api`.  
**Development:** `typescript`, `vite`, `@vitejs/plugin-react`, `@tauri-apps/cli`, ESLint và plugin TypeScript/React phù hợp, `vitest`, `jsdom`, `@testing-library/react`.

Phase 1 chưa cần Tailwind, shadcn/ui, Radix, Lucide, Motion, React Router, Zustand, TanStack Query hoặc TanStack Virtual. Đưa chúng vào đúng phase khi có yêu cầu UI/feature sử dụng; tránh dependency chưa dùng.

### Dependency Rust

- `tauri` 2.x và `tauri-build` 2.x.
- `serde` với derive và `serde_json` cho IPC contracts.
- `sqlx` với các feature SQLite, Tokio runtime, migrations và macros cần cho embedded migration.
- `thiserror` hoặc error enum tương đương để chuyển lỗi nội bộ sang lỗi IPC có cấu trúc.
- `tempfile` chỉ ở dev-dependencies nếu cần database tạm cho test.

Phase 1 chưa cần `reqwest`, `zip`, Tauri filesystem/HTTP plugins, Tokio multi-thread runtime tùy chỉnh hoặc downloader dependencies.

### Bootstrap commands tham khảo (PowerShell)

```powershell
node --version
npm --version
rustc --version
cargo --version
rustup show

npm create vite@latest manga-downloader-1989 -- --template react-ts
cd manga-downloader-1989
npm install
npm install @tauri-apps/api
npm install -D @tauri-apps/cli
npx tauri init
```

Nếu repository đã tồn tại, không scaffold đè lên file của người dùng; kiểm tra và bổ sung cấu hình tại chỗ. Thiết lập Tauri `devUrl` và Vite port rõ ràng (ví dụ `1420` với `strictPort: true`), cùng `frontendDist` trỏ đến `../dist`.

## 6. Contracts TypeScript/Rust

### Frontend contracts — `src/shared/api/contracts.ts`

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

### IPC wrapper — `src/shared/api/desktop.ts`

- Import `invoke` từ `@tauri-apps/api/core`.
- Export `getAppInfo(): Promise<AppInfo>` gọi command `app_get_info`.
- Export `getAppHealth(): Promise<AppHealth>` gọi command `app_health_check`.
- Không để component gọi `invoke()` trực tiếp.
- Gom/chuẩn hóa lỗi IPC tại một chỗ nếu cần; không ép kiểu error thành `any`.

### Rust equivalents

Đặt struct tương ứng ở module system/domain thích hợp; derive `Serialize`, dùng `#[serde(rename_all = "camelCase")]`. Hai command phải được đăng ký tường minh trong `tauri::generate_handler!`. `app_health_check` chỉ trả `database: "ready"` sau khi DB pool được khởi tạo thành công. Nếu init thất bại, dừng khởi động với lỗi có thể chẩn đoán trong log phát triển; không báo trạng thái khỏe giả.

Ví dụ kiểu lỗi wire-format (tùy chọn `tag/content` cũng được chấp nhận nếu TypeScript contract đồng bộ):

```json
{
  "code": "DATABASE_INIT_FAILED",
  "message": "Không thể khởi tạo cơ sở dữ liệu local.",
  "retryable": true
}
```

Không đưa chi tiết đường dẫn nội bộ, query SQL hoặc stack trace vào `message` trả về frontend.

## 7. SQLite migration tối thiểu

Tạo `src-tauri/migrations/0001_init.sql` với duy nhất bảng cài đặt tổng quát:

```sql
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Quy ước:

- `key` là định danh ổn định; `value_json` lưu JSON đã serialize; `updated_at` là UTC ISO-8601 do Rust gán khi ghi.
- Không lưu setting thực tế trong migration; không cần seed dữ liệu cho Phase 1.
- Không tạo bảng Manga/chapter/source/download trước khi data model tương ứng được thiết kế ở phase chuyên biệt.
- Migration là append-only sau khi đã phát hành; thay đổi schema thì thêm migration mới.
- Khởi tạo pool từ đường dẫn app data do Tauri cung cấp, tạo thư mục nếu thiếu, bật foreign keys và dùng SQLx migrator nhúng. Không hard-code `./data` hoặc đường dẫn người dùng.
- Bật chế độ SQLite/WAL chỉ khi cấu hình connection pool nhất quán và có lý do; không thêm cấu hình storage khó đảo ngược trong phase nền tảng.

## 8. Tasks theo thứ tự

### Task 1 — Kiểm tra repository và công cụ

**Tạo/sửa:** `README.md` (nếu thiếu), `.gitignore` (nếu thiếu).  
**Việc làm:** kiểm tra repo hiện có trước khi scaffold; xác nhận Node, npm, Rust stable MSVC, C++ Build Tools, Windows SDK và WebView2. Không xóa hoặc ghi đè user files.

**Nghiệm thu:** README liệt kê prerequisite chính xác; `.gitignore` loại `node_modules/`, `dist/`, `target/`, database runtime và build outputs nhưng không bỏ lockfiles.

### Task 2 — Scaffold frontend và Tauri

**Tạo/sửa:** `package.json`, lockfile, các `tsconfig*.json`, `vite.config.ts`, `index.html`, `src/main.tsx`, `src/vite-env.d.ts`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`.

**Việc làm:** cấu hình React + TypeScript + Vite; tạo Tauri 2 desktop entry; chốt port development; cấu hình bundle identifier duy nhất theo reverse-DNS, tên app `Manga Downloader 1989`, build Windows x64. Không thêm feature plugins chưa dùng.

**Nghiệm thu:** `npm install` không lỗi; Vite build thành công; Tauri dev khởi chạy một cửa sổ desktop local; build không phụ thuộc CDN hoặc service bên ngoài.

### Task 3 — Cấu trúc frontend, shell và tokens

**Tạo/sửa:** `src/app/App.tsx`, `src/features/system/SystemStatus.tsx`, `src/shared/api/contracts.ts`, `src/shared/api/desktop.ts`, `src/styles/tokens.css`, `src/styles/globals.css`.

**Việc làm:** tạo shell tối thiểu dark-first theo các token nền tảng bên dưới. Shell hiển thị tên app, version và trạng thái backend/DB qua IPC; trạng thái loading/error phải phân biệt được.

| Token | Giá trị |
|---|---|
| Background | `#101016` |
| Sidebar | `#171720` |
| Surface | `#20202C` |
| Elevated | `#292938` |
| Primary | `#A78BFA` |
| Hover | `#8B5CF6` |
| Text | `#F5F3FF` |
| Muted | `#A1A1B5` |

**Nghiệm thu:** giao diện dùng CSS variables thay vì lặp màu rải rác; không có link/nút giả cho tính năng chưa triển khai; app còn đọc được ở kích thước cửa sổ nhỏ.

### Task 4 — Backend modules và database bootstrap

**Tạo/sửa:** `src-tauri/src/app_state.rs`, `src-tauri/src/error.rs`, `src-tauri/src/db/mod.rs`, `src-tauri/migrations/0001_init.sql`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`.

**Việc làm:** tạo `SqlitePool` từ app data directory qua Tauri path API; tạo thư mục DB khi chưa tồn tại; chạy migration trước khi UI báo healthy; đăng ký state pool. Dùng async SQLx/Tauri runtime, không blocking database work trên UI thread.

**Nghiệm thu:** lần chạy đầu tự tạo DB và migration metadata; lần chạy sau mở lại thành công; database chỉ nằm trong app data; lỗi init được hiển thị/log có kiểm soát; app không tạo file database cạnh executable.

### Task 5 — Command layer và typed IPC

**Tạo/sửa:** `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/system.rs`, `src-tauri/src/error.rs`, `src-tauri/src/lib.rs`, `src/shared/api/contracts.ts`, `src/shared/api/desktop.ts`, `src/features/system/SystemStatus.tsx`.

**Việc làm:** triển khai `app_get_info` và `app_health_check`; đăng ký đúng hai command; map Rust structs/errors sang camelCase JSON; gọi từ frontend qua wrapper.

**Nghiệm thu:** version/name từ Rust hiện trên shell; health check chỉ báo ready khi pool tồn tại; lỗi command tạo rejected promise/error state được xử lý; không có `invoke` nằm ngoài wrapper.

### Task 6 — Security và Tauri capabilities

**Tạo/sửa:** `src-tauri/capabilities/default.json`, `src-tauri/tauri.conf.json`, có thể `src-tauri/build.rs` nếu dùng command allowlist.

**Việc làm:** giới hạn capability cho cửa sổ `main`; chỉ cấp core/plugin permission thật sự cần. Không thêm filesystem, shell, dialog, HTTP plugin permission, remote origins hay wildcard host access trong Phase 1. Frontend không bật `withGlobalTauri`. Giữ CSP bật và chặt; cho phép asset local và IPC cần Tauri, không cho script/CDN từ xa, không đặt `*` trong directives. Dev CSP có thể cho dev server/HMR local riêng; không nới production CSP để chữa lỗi cấu hình.

Các command Rust tự viết phải được đăng ký rõ. Nếu cấu hình build của Tauri hỗ trợ command allowlist qua `AppManifest`, giới hạn danh sách cho `app_get_info` và `app_health_check`; xác nhận hành vi trên phiên bản Tauri đã khóa.

**Nghiệm thu:** app dev/build mở được; chỉ cửa sổ `main` có capability; không có plugin permission dư; CSP không có wildcard hoặc remote script source.

### Task 7 — Lint, type-check, test và build scripts

**Tạo/sửa:** `eslint.config.js`, `vitest.config.ts` (nếu tách riêng), `tests/app.test.tsx`, test Rust trong `db/mod.rs` hoặc module test phù hợp, `package.json` scripts.

**Việc làm:** tạo tối thiểu một frontend smoke test cho shell/API state rendering và một Rust test khởi tạo SQLite tạm rồi xác nhận `app_settings` tồn tại sau migration. Test không gọi mạng và không chạm user database.

**Nghiệm thu:** các lệnh bên dưới có script tương ứng, kết thúc với exit code thành công trong môi trường Windows đã cài đủ prerequisite; lockfiles được commit và repo không chứa database test/runtime.

### Task 8 — README và bàn giao

**Tạo/sửa:** `README.md`.

**Việc làm:** ghi stack, prerequisite Windows, lệnh cài/chạy, cấu trúc cơ bản, app data/DB location theo Tauri API, cách chạy quality gates và giới hạn phạm vi Phase 1. Ghi chính xác mọi bước mà agent đã thực hiện.

**Nghiệm thu:** developer mới có thể làm theo README; không ghi rằng tính năng tìm kiếm, import, downloader hoặc reader đã hoạt động.

## 9. Lệnh build, lint và test

Khai báo npm scripts tối thiểu:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "lint": "eslint .",
    "typecheck": "tsc -b --pretty false",
    "test": "vitest run",
    "test:watch": "vitest",
    "tauri": "tauri"
  }
}
```

Lệnh kiểm tra Phase 1 trên PowerShell:

```powershell
npm ci
npm run lint
npm run typecheck
npm run test
npm run build
npm run tauri dev
npm run tauri build
```

`npm run tauri build` tạo executable/bundle release Windows. Nếu agent không có Windows/MSVC/WebView2 phù hợp để chạy build desktop hoặc test, phải ghi rõ lệnh nào đã chạy, lệnh nào bị chặn và lý do; không đánh dấu pass khi chưa chạy.

## 10. Security và ranh giới cho các phase sau

- Không dùng `shell:default`, `fs:default`, `http:default` hoặc capability wildcard trong nền tảng Phase 1.
- Chỉ Rust truy cập SQLite; frontend gửi dữ liệu qua command contracts đã validate.
- Không load JavaScript từ xa hoặc tắt CSP.
- Future URL import/search cần danh sách host nguồn hỗ trợ và validation ở backend. Chặn scheme ngoài HTTP/HTTPS, loopback/private/link-local/reserved IP, địa chỉ metadata cloud; kiểm tra lại redirect target và DNS resolution để giảm SSRF. Giới hạn timeout, kích thước response và số redirect.
- Future downloader cần ghi file trong thư mục app-managed hoặc do người dùng chọn với quyền tối thiểu; canonicalize/validate path và tên file để chặn traversal.
- Không log cookies, authorization headers, token, full page payload hoặc đường dẫn riêng khi không cần.
- Nội dung HTML/metadata từ nguồn là dữ liệu không tin cậy: render text an toàn, không inject HTML thô.

Các biện pháp URL/network ở trên là yêu cầu thiết kế cho phase tương lai; không yêu cầu implement crawler trong Phase 1.

## 11. Definition of Done

- [ ] Repo chạy được trên Windows theo README, với package manager/lockfile nhất quán.
- [ ] Tauri 2 desktop window khởi động; frontend là React + TypeScript + Vite.
- [ ] Shell dùng Dark Purple tokens và hiển thị thông tin app cùng trạng thái hệ thống.
- [ ] Frontend IPC đi qua một wrapper typed; hai command system đã được đăng ký rõ ràng.
- [ ] SQLite dùng app data directory, khởi tạo ổn định và áp dụng migration `0001_init.sql`.
- [ ] Capability/CSP không cấp quyền filesystem/network/shell không cần thiết.
- [ ] Lint, type-check, tests, frontend build và Tauri Windows build đều pass; ngoại lệ môi trường được ghi rõ.
- [ ] `README.md` và handoff mô tả đúng phần đã hoàn thành, phần chưa làm và các lệnh đã chạy.
- [ ] Không có tính năng ngoài Phase 1 được mô tả như đã triển khai.

## 12. Handoff review

Khi hoàn tất, Antigravity cần trả về:

1. Tóm tắt thay đổi theo frontend, Rust/SQLite, security và tooling.
2. Danh sách file chính đã tạo/sửa.
3. Kết quả thực tế của từng lệnh lint, type-check, test, frontend build và Tauri build; ghi `not run` cùng lý do nếu thiếu môi trường.
4. Vị trí DB runtime theo API app data đã dùng (không cần ghi đường dẫn riêng của máy người dùng vào repo).
5. Các giả định hoặc vấn đề còn mở cần quyết định trước Phase 2.
6. Xác nhận không triển khai URL import, search, download, library hay offline reader ở phase này.

Reviewer cần kiểm tra: project có ghi đè file cũ không; async DB init có block UI không; migration được embedded và idempotent theo SQLx history không; frontend có gọi command ngoài wrapper không; có permission/CSP rộng hoặc dependency không dùng không; test có dùng file DB người dùng thật không; README/handoff có khẳng định quá mức không.

## 13. Prompt ngắn giao Antigravity

```text
Hãy đọc toàn bộ PHASE_01_IMPLEMENTATION_PLAN.md và triển khai đúng Phase 01 — Project Foundation & Architecture cho Manga Downloader 1989 trong repo hiện tại. Trước tiên kiểm tra repo và giữ nguyên mọi file người dùng; không scaffold đè. Dựng nền React + TypeScript + Vite + Tauri 2 + Rust/SQLx trên Windows, typed IPC tối thiểu, SQLite app-data + migration, Dark Purple shell, security capabilities, lint/typecheck/test/build scripts và README như brief. Chỉ làm phạm vi Phase 1; không thêm URL import, search, downloader, library hoặc reader. Chạy các quality gates có thể chạy được, ghi rõ pass/fail/not run và lý do, rồi bàn giao theo mục “Handoff review”. Không tuyên bố lệnh chưa chạy là pass.
```

## 14. Tài liệu chính thức tham khảo

- [Tauri v2 — Windows prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri v2 — Create a project](https://v2.tauri.app/start/create-project/)
- [Tauri v2 — Capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri v2 — Calling Rust from the frontend](https://v2.tauri.app/develop/calling-rust/)
- [Tauri v2 — Content Security Policy](https://v2.tauri.app/security/csp/)
- [Tauri v2 — Windows installer/build](https://v2.tauri.app/distribute/windows-installer/)
- [Vite — Getting started and Node.js requirements](https://vite.dev/guide/)
- [SQLx — SQLite driver](https://docs.rs/sqlx/latest/sqlx/sqlite/)
- [SQLx — Migrations](https://docs.rs/sqlx/latest/sqlx/migrate/)

Các phiên bản và requirements thay đổi theo thời gian. Agent cần đối chiếu tài liệu chính thức tại lúc scaffold, chọn các phiên bản tương thích và lưu lockfiles; nếu tài liệu hiện hành khác với hướng dẫn phiên bản ở trên, cập nhật README và nêu rõ thay đổi.
