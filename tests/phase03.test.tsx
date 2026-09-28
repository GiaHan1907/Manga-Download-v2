import { beforeEach, describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "../src/app/router";
import { resetMockState, services } from "../src/services/provider";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    isMaximized: vi.fn().mockResolvedValue(false),
  })),
}));

import { invoke } from "@tauri-apps/api/core";

const DEFAULT_SETTINGS = {
  defaultReaderMode: "single",
  defaultReadingDirection: "rtl",
  chapterConcurrency: 2,
  imageConcurrency: 4,
  outputFormat: "cbz",
  notificationsEnabled: true,
  closeToTray: false,
};

function installBackendMock() {
  const state = { settings: { ...DEFAULT_SETTINGS } };
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" });
      case "app_health_check":
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 2 });
      case "library_stats":
        return Promise.resolve({
          savedMangas: 3,
          downloadedChapters: 7,
          localSizeBytes: 12345,
          favorites: 2,
        });
      case "app_list_local_download_tasks":
        return Promise.resolve({ items: [], total: 0, limit: 5, offset: 0 });
      case "storage_status":
        return Promise.resolve({
          libraryBytes: 12345,
          cacheBytes: 0,
          stagingDirs: 0,
          activeTasks: 0,
          rootAvailable: true,
          customRoot: false,
          rootLinkOk: true,
          approximate: true,
        });
      case "app_get_settings":
        return Promise.resolve({ ...state.settings });
      case "app_update_settings": {
        const patch = (args?.input ?? {}) as Record<string, unknown>;
        Object.assign(state.settings, patch);
        return Promise.resolve({ ...state.settings });
      }
      case "app_reset_settings":
        state.settings = { ...DEFAULT_SETTINGS };
        return Promise.resolve({ ...state.settings });
      case "app_list_local_mangas":
      case "app_list_favorite_mangas":
      case "library_list":
        return Promise.resolve({ items: [], total: 0, limit: 24, offset: 0 });
      case "library_get_downloaded_chapters":
        return Promise.resolve({ chapters: [] });
      case "list_supported_sources":
        return Promise.resolve([
          {
            id: "mangadex",
            displayName: "MangaDex",
            capabilities: ["search", "resolveUrl", "details"],
            status: "ready",
            attribution: null,
            enabled: true,
            builtin: true,
            baseUrl: null,
          },
        ]);
      case "set_source_enabled":
        return Promise.resolve(true);
      default:
        return Promise.reject(new Error(`Unknown command ${cmd}`));
    }
  });
  return state;
}

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  resetMockState();
  installBackendMock();
});

describe("T03-01/T03-02 Home journey (real data since Phase 08)", () => {
  it("navigates from the real Discover CTA to Discover", async () => {
    renderAt("/");
    const explore = await screen.findByRole("link", { name: "Khám phá" });
    fireEvent.click(explore);
    expect(await screen.findByRole("heading", { name: "Khám phá" })).toBeDefined();
  });

  it("renders stats from the real library_stats command, with no fixture claims", async () => {
    renderAt("/");
    const stats = await screen.findByLabelText("Thống kê local");
    expect(within(stats).getByText("7")).toBeDefined();
    expect(within(stats).getByText("3")).toBeDefined();
    expect(screen.queryByText(/dữ liệu mẫu/)).toBeNull();
  });

});

describe("T03-03/T03-04 Discover search and demo import", () => {
  it("debounced search filters fixtures and supports empty state", async () => {
    renderAt("/discover?tab=demo");
    const input = await screen.findByPlaceholderText(/Nhập tên, tác giả/);
    fireEvent.change(input, { target: { value: "giấy" } });
    expect(await screen.findByText("Kỷ Nguyên Giấy")).toBeDefined();

    fireEvent.change(input, { target: { value: "hong-co-ket-qua" } });
    expect(await screen.findByText("Không có kết quả mẫu")).toBeDefined();
  });

  it("resolves the sample demo URL and rejects others", async () => {
    renderAt("/discover?tab=demo-url");
    const input = await screen.findByPlaceholderText(/demo\.manga1989/);
    fireEvent.change(input, { target: { value: "https://example.org/x" } });
    fireEvent.click(screen.getByRole("button", { name: "Kiểm tra URL" }));
    expect(await screen.findByText(/Nguồn không được hỗ trợ/)).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: "Dùng URL mẫu" }));
    fireEvent.click(screen.getByRole("button", { name: "Kiểm tra URL" }));
    const result = await screen.findByLabelText("Kết quả import demo");
    expect(within(result).getByText("Hành Trình Ngôi Sao")).toBeDefined();
    fireEvent.click(within(result).getByRole("link", { name: "Mở chi tiết truyện" }));
    expect(await screen.findByRole("heading", { name: "Chi tiết truyện" })).toBeDefined();
  });
});

describe("T03-05/T03-06 demo detail, queue, favorites and local boundary", () => {
  it("selects chapters, queues with added/skipped counts", async () => {
    renderAt("/manga/manga-001");
    const checkboxes = await screen.findAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);
    fireEvent.click(checkboxes[2]);
    fireEvent.click(screen.getByRole("button", { name: "Add selected to demo queue" }));
    expect((await screen.findByRole("status")).textContent).toMatch(/thêm mới 1, bỏ qua 1/);
  });

  it("demo favorite toggles in demo sandbox; Favorites page is Local-only (Phase 04 boundary)", async () => {
    renderAt("/manga/manga-003");
    const star = await screen.findByRole("button", { name: /Yêu thích \(demo\)/ });
    fireEvent.click(star);
    await waitFor(() => expect(star.getAttribute("aria-pressed")).toBe("true"));

    fireEvent.click(screen.getByRole("link", { name: "Yêu thích" }));
    expect(await screen.findByText("Chưa có mục yêu thích local")).toBeDefined();
  });
});

describe("T03-08 Reader (dev sandbox)", () => {
  it("navigates pages with bounds and records mock progress", async () => {
    renderAt("/reader/manga-001/manga-001-ch-01");
    expect(await screen.findByText(/Trang 1\/4/)).toBeDefined();

    const next = screen.getByRole("button", { name: "Trang sau" });
    fireEvent.click(next);
    expect(await screen.findByText(/Trang 2\/4/)).toBeDefined();

    const prev = screen.getByRole("button", { name: "Trang trước" });
    fireEvent.click(prev);
    fireEvent.click(prev);
    await waitFor(() => expect((screen.getByRole("button", { name: "Trang trước" }) as HTMLButtonElement).disabled).toBe(true));

    fireEvent.click(next);
    await waitFor(async () => {
      const progress = await services.reader.getProgress("manga-001");
      expect(progress?.page).toBe(1);
    });
  });

  it("shows fallback for chapters without sample pages", async () => {
    renderAt("/reader/manga-002/manga-002-ch-01");
    expect(await screen.findByText("Chương này chưa có trang mẫu")).toBeDefined();
  });
});

describe("T03-09 error and empty states", () => {
  it("unknown manga id shows recoverable error", async () => {
    renderAt("/manga/manga-999");
    expect(await screen.findByText("Không tìm thấy manga này")).toBeDefined();
  });
});

describe("T03-08/T04 Settings and Sources", () => {
  it("rejects out-of-bounds settings client-side", async () => {
    renderAt("/settings");
    const chapterInput = await screen.findByLabelText(/Chương đồng thời/);
    fireEvent.change(chapterInput, { target: { value: "9" } });
    fireEvent.click(screen.getByRole("button", { name: "Lưu thay đổi" }));
    expect((await screen.findByRole("status")).textContent).toMatch(/số nguyên từ 1 đến 5/);
  });

  it("valid settings save through IPC and persist in backend mock", async () => {
    const backend = installBackendMock();
    renderAt("/settings");
    const chapterInput = await screen.findByLabelText(/Chương đồng thời/);
    fireEvent.change(chapterInput, { target: { value: "4" } });
    const saveButton = screen.getByRole("button", { name: "Lưu thay đổi" });
    fireEvent.click(saveButton);
    expect((await screen.findByRole("status")).textContent).toMatch(/Đã lưu/);
    await waitFor(() => expect(backend.settings.chapterConcurrency).toBe(4));
  });

  it("source toggle affects enabled list", async () => {
    renderAt("/settings/sources");
    const toggles = await screen.findAllByRole("button", { name: "Tắt nguồn" });
    fireEvent.click(toggles[0]);
    expect(await screen.findByText("disabled")).toBeDefined();
    const addSource = screen.getByRole("button", { name: /Thêm nguồn/ });
    expect((addSource as HTMLButtonElement).disabled).toBe(true);
  });

  it("disabling a source removes its manga from Discover results (§3.7)", async () => {
    renderAt("/settings/sources");
    const rows = await screen.findAllByRole("button", { name: "Tắt nguồn" });
    const betaToggle = rows[1];
    const betaLabel = betaToggle.closest(".source-row")?.textContent ?? "";
    expect(betaLabel).toContain("Beta");
    fireEvent.click(betaToggle);

    fireEvent.click(screen.getByRole("link", { name: "Khám phá" }));
    fireEvent.click(await screen.findByRole("tab", { name: /Demo sandbox/ }));
    const input = await screen.findByPlaceholderText(/Nhập tên, tác giả/);

    fireEvent.change(input, { target: { value: "kỷ nguyên" } });
    expect(await screen.findByText("Không có kết quả mẫu")).toBeDefined();

    fireEvent.change(input, { target: { value: "hanh trinh" } });
    expect(await screen.findByText("Hành Trình Ngôi Sao")).toBeDefined();
  });
});
