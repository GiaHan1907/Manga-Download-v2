import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
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

type Backend = {
  calls: Array<[string, Record<string, unknown> | undefined]>;
  searches: Array<{ query: string; searchedAt: string }>;
};

let backend: Backend;

const item = (id: string, title: string, chapter: string) => ({
  sourceId: "truyenqq",
  sourceMangaId: id,
  sourceDisplayName: "TruyenQQ",
  title,
  author: null,
  coverKey: null,
  status: "ongoing",
  latestChapter: chapter,
  latestTime: "2 giờ trước",
});

function ranking(period: unknown) {
  // Distinct rows per period so a tab that failed to refetch cannot pass.
  const suffix = String(period);
  return [
    item(`one-${suffix}`, `Đứng đầu ${suffix}`, "Chapter 10"),
    item(`two-${suffix}`, `Hạng hai ${suffix}`, "Chapter 20"),
  ];
}

beforeEach(() => {
  backend = { calls: [], searches: [{ query: "naruto", searchedAt: "2026-09-30T00:00:00.000Z" }] };
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    backend.calls.push([cmd, args]);
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({ name: "Manga Downloader 1989", version: "1.0.0", platform: "windows" });
      case "app_health_check":
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 10 });
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
        return Promise.resolve({ featured: [], newest: [], popular: [] });
      case "search_manga":
        return Promise.resolve({ items: [], sourceErrors: [], hasMore: false });
      case "list_source_genres":
        return Promise.resolve([
          { slug: "action-26", label: "Action" },
          { slug: "huyen-huyen-468", label: "Huyền Huyễn" },
        ]);
      case "browse_manga":
        return Promise.resolve({
          items: [
            {
              sourceId: "truyenqq",
              sourceMangaId: "dao-bich-canh-2800",
              sourceDisplayName: "TruyenQQ",
              title: "Đao Bích Cảnh",
              author: null,
              coverKey: null,
              status: "ongoing",
            },
          ],
          sourceErrors: [],
          hasMore: false,
        });
      case "get_home_ranking":
        return Promise.resolve(ranking(args?.period));
      case "search_history_list":
        return Promise.resolve(backend.searches);
      case "search_history_record": {
        const query = String(args?.query);
        backend.searches = [{ query, searchedAt: "now" }, ...backend.searches.filter((s) => s.query !== query)];
        return Promise.resolve(backend.searches);
      }
      case "search_history_remove":
        backend.searches = backend.searches.filter((s) => s.query !== args?.query);
        return Promise.resolve(backend.searches);
      case "search_history_clear":
        backend.searches = [];
        return Promise.resolve(0);
      case "app_list_read_history":
        return Promise.resolve({
          items: [
            {
              mangaId: "m-1",
              mangaTitle: "Ninja Loạn Thị",
              chapterId: "c-9",
              chapterTitle: "Chương 9",
              chapterNumber: 9,
              readAt: "2026-09-30T00:00:00.000Z",
            },
          ],
          total: 1,
          limit: 20,
          offset: 0,
        });
      default:
        return Promise.reject(new Error(`Unknown command ${cmd}`));
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

describe("Discover Phase 2 — ranking screen", () => {
  it("shows the source's own order and refetches per period", async () => {
    renderAt("/ranking");
    expect(await screen.findByText("Đứng đầu day")).toBeDefined();
    expect(screen.getByText("Hạng hai day")).toBeDefined();

    fireEvent.click(screen.getByRole("tab", { name: "Theo tháng" }));
    expect(await screen.findByText("Đứng đầu month")).toBeDefined();
    const periods = backend.calls
      .filter(([cmd]) => cmd === "get_home_ranking")
      .map(([, args]) => args?.period);
    expect(periods).toEqual(["day", "month"]);
  });

  it("never invents a view count next to a rank", async () => {
    renderAt("/ranking");
    const list = await screen.findByRole("list");
    expect(within(list).queryByText(/lượt xem|views|★/i)).toBeNull();
  });
});

describe("Discover Phase 2 — catalog filters", () => {
  it("browses a genre through the source's own catalog instead of faking a keyword filter", async () => {
    renderAt("/discover");
    const genreSelect = await screen.findByLabelText(/Thể loại/);
    fireEvent.change(genreSelect, { target: { value: "action-26" } });

    expect(await screen.findByText("Đao Bích Cảnh")).toBeDefined();
    const call = backend.calls.find(([cmd]) => cmd === "browse_manga");
    expect(call?.[1]).toMatchObject({ sourceId: "truyenqq", genre: "action-26", page: 1 });
    // The keyword box cannot combine with a catalog filter, so it is disabled
    // rather than left there pretending to work.
    expect((screen.getByLabelText("Tìm kiếm truyện") as HTMLInputElement).disabled).toBe(true);
  });

  it("hides the genre filter entirely when the source publishes no list", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_source_genres") {
        return Promise.reject({ code: "UNSUPPORTED_OPERATION", message: "Nguồn không hỗ trợ.", retryable: false });
      }
      if (cmd === "get_home_feed") {
        return Promise.resolve({ featured: [], newest: [], popular: [] });
      }
      if (cmd === "list_supported_sources") {
        return Promise.resolve([]);
      }
      return Promise.resolve({ items: [], sourceErrors: [], hasMore: false });
    });
    renderAt("/discover");
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(screen.queryAllByLabelText(/Thể loại/)).toHaveLength(0);
  });
});

describe("Discover Phase 2 — recent searches", () => {
  it("records a committed search and offers it back as a chip", async () => {
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/Nhập tên truyện/);
    fireEvent.change(input, { target: { value: "one piece" } });
    fireEvent.click(screen.getByRole("button", { name: "Tìm kiếm" }));

    const call = backend.calls.find(([cmd]) => cmd === "search_history_record");
    expect(call?.[1]).toEqual({ query: "one piece" });
    expect(await screen.findByRole("button", { name: "one piece" })).toBeDefined();
  });

  it("removes one query through IPC instead of hiding it locally", async () => {
    renderAt("/history");
    fireEvent.click(await screen.findByRole("button", { name: "Xóa naruto khỏi lịch sử" }));
    await vi.waitFor(() => {
      expect(backend.calls.some(([cmd]) => cmd === "search_history_remove")).toBe(true);
    });
    expect(await screen.findByText("Chưa tìm kiếm gì")).toBeDefined();
    expect(screen.queryByText("naruto")).toBeNull();
  });

  it("lists read chapters separately from search queries", async () => {
    renderAt("/history");
    expect(await screen.findByText("Ninja Loạn Thị")).toBeDefined();
    expect(screen.getByText("Chương 9")).toBeDefined();
    expect(screen.getByRole("link", { name: /Ninja Loạn Thị/ }).getAttribute("href")).toContain("/read/c-9");
  });
});
