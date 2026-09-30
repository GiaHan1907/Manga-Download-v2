import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "../src/app/router";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    isMaximized: vi.fn().mockResolvedValue(false),
  })),
}));

import { invoke } from "@tauri-apps/api/core";

type Backend = { calls: Array<[string, Record<string, unknown> | undefined]> };

let backend: Backend;

const card = (id: string, title: string) => ({
  sourceId: "truyenqq",
  sourceMangaId: id,
  sourceDisplayName: "TruyenQQ",
  title,
  author: null,
  coverKey: null,
  status: "ongoing",
  latestChapter: "Chapter 5",
  latestTime: "1 giờ trước",
});

/// The browse payload names itself after the filters it was asked for, so a
/// test can prove WHICH request the screen actually made, not only that a
/// card appeared.
function catalogTitle(args: Record<string, unknown> | undefined) {
  const part = (key: string) => (args?.[key] ? String(args[key]) : "-");
  return `Danh mục ${part("genre")}/${part("status")}/${part("sort")}`;
}

beforeEach(() => {
  backend = { calls: [] };
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    backend.calls.push([cmd, args]);
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({ name: "Manga Downloader 1989", version: "1.0.0", platform: "windows" });
      case "app_health_check":
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 11 });
      case "list_supported_sources":
        return Promise.resolve([
          {
            id: "truyenqq",
            displayName: "TruyenQQ",
            capabilities: ["search", "resolveUrl", "details", "pages"],
            status: "ready",
            attribution: null,
            enabled: true,
            builtin: true,
            baseUrl: "https://truyenqqko.com/",
          },
        ]);
      case "get_home_feed":
        return Promise.resolve({
          featured: [card("hot-1", "Bộ đang hot"), card("hot-2", "Bộ thứ hai")],
          newest: [card("new-1", "Bộ vừa cập nhật")],
          popular: [card("pop-1", "Bộ nhiều người xem")],
        });
      case "search_manga":
        return Promise.resolve({ items: [], sourceErrors: [], hasMore: false });
      case "list_source_genres":
        return Promise.resolve([
          { slug: "action-26", label: "Action" },
          { slug: "huyen-huyen-468", label: "Huyền Huyễn" },
        ]);
      case "browse_manga":
        return Promise.resolve({
          items: [card("row-1", catalogTitle(args))],
          sourceErrors: [],
          hasMore: true,
        });
      case "get_home_ranking":
        return Promise.resolve([card("rank-1", "Đứng đầu")]);
      case "search_history_list":
        return Promise.resolve([]);
      case "search_history_record":
        return Promise.resolve([]);
      case "library_list":
        return Promise.resolve({ items: [], total: 0, page: 1, pageSize: 20, hasMore: false });
      case "library_stats":
        return Promise.resolve({
          totalMangas: 0,
          totalChapters: 0,
          downloadedMangas: 0,
          reading: 0,
          completed: 0,
          paused: 0,
          missing: 0,
          favoriteCount: 0,
          queued: 0,
        });
      case "library_genres":
        return Promise.resolve([]);
      case "storage_status":
        return Promise.resolve({
          libraryBytes: 0,
          cacheBytes: 0,
          stagingDirs: 0,
          activeTasks: 0,
          rootAvailable: true,
          rootPath: "D:\\Manga",
          volumeTotalBytes: null,
          volumeFreeBytes: null,
        });
      case "app_list_read_history":
        return Promise.resolve({ items: [] });
      case "get_settings":
        return Promise.resolve({});
      default:
        return Promise.resolve({ items: [], sourceErrors: [], hasMore: false });
    }
  });
});

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

function lastBrowse(): Record<string, unknown> | undefined {
  const calls = backend.calls.filter(([cmd]) => cmd === "browse_manga");
  return calls[calls.length - 1]?.[1];
}

describe("Discover catalog sorting", () => {
  it("orders a genre page once the genre that owns the sort is chosen", async () => {
    renderAt("/discover");
    const sortSelect = (await screen.findByLabelText(/Sắp xếp/)) as HTMLSelectElement;
    // With no genre the control cannot do anything, so it says so by being
    // disabled rather than accepting a choice that goes nowhere.
    expect(sortSelect.disabled).toBe(true);

    fireEvent.change(screen.getByLabelText(/Thể loại/), { target: { value: "action-26" } });
    expect(await screen.findByText("Danh mục action-26/-/-")).toBeDefined();
    expect(sortSelect.disabled).toBe(false);

    fireEvent.change(sortSelect, { target: { value: "views-desc" } });
    expect(await screen.findByText("Danh mục action-26/-/views-desc")).toBeDefined();
    expect(lastBrowse()).toMatchObject({ genre: "action-26", sort: "views-desc", page: 1 });
  });

  it("sends genre, status and sort together on one request", async () => {
    renderAt("/discover?genre=action-26&status=completed&sort=views-desc");
    expect(await screen.findByText("Danh mục action-26/completed/views-desc")).toBeDefined();
    expect(lastBrowse()).toMatchObject({
      sourceId: "truyenqq",
      genre: "action-26",
      status: "completed",
      sort: "views-desc",
      page: 1,
    });
  });

  it("drops the pending sort when the genre goes away", async () => {
    renderAt("/discover?genre=action-26&sort=views-desc");
    expect(await screen.findByText("Danh mục action-26/-/views-desc")).toBeDefined();

    fireEvent.change(screen.getByLabelText(/Thể loại/), { target: { value: "" } });
    const sortSelect = (await screen.findByLabelText(/Sắp xếp/)) as HTMLSelectElement;
    expect(sortSelect.value).toBe("");
    expect(sortSelect.disabled).toBe(true);
    // With no filter left the screen is back in keyword mode, not a sort-only browse.
    expect((screen.getByLabelText("Tìm kiếm truyện") as HTMLInputElement).disabled).toBe(false);
  });

  it("ignores a sort deep link that carries no genre", async () => {
    renderAt("/discover?sort=views-desc");
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(backend.calls.some(([cmd]) => cmd === "browse_manga")).toBe(false);
    expect((screen.getByLabelText("Tìm kiếm truyện") as HTMLInputElement).disabled).toBe(false);
  });

  it("carries the sort into every page it loads after the first", async () => {
    renderAt("/discover?genre=action-26&sort=views-desc");
    expect(await screen.findByText("Danh mục action-26/-/views-desc")).toBeDefined();
    // Dropping the sort here would append page 2 of the DEFAULT order under a
    // heading that still claims the list is by views.
    fireEvent.click(screen.getByRole("button", { name: "Xem thêm" }));
    await waitFor(() => {
      expect(lastBrowse()).toMatchObject({ page: 2, genre: "action-26", sort: "views-desc" });
    });
  });
});

describe("Navigation wiring", () => {
  it("lists the sources registry in the sidebar", async () => {
    renderAt("/");
    await screen.findByLabelText("Truyện đang hot");
    expect(screen.getByRole("link", { name: "Nguồn truyện" })).toBeDefined();
  });

  it("links every Home 'Xem tất cả' to a screen that shows the whole list", async () => {
    renderAt("/");
    await screen.findByLabelText("Truyện đang hot");
    const hrefs = screen
      .getAllByText("Xem tất cả")
      .map((node) => node.closest("a")?.getAttribute("href") ?? "");
    // MemoryRouter emits bare paths; the app's HashRouter prefixes them with #.
    expect(hrefs).toContain("/ranking");
    expect(hrefs).toContain("/discover?status=new");
    expect(hrefs).not.toContain("/discover");
  });
});
