import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "../src/app/router";

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
import { normalizeIpcError } from "../src/shared/api/desktop";

const TTT_SOURCE = {
  id: "madara-truyentuoitho-com",
  displayName: "Truyen Tuoi Tho",
  capabilities: ["search", "resolveUrl", "details"],
  status: "ready",
  attribution: "Site do bên thứ ba vận hành",
  enabled: true,
  builtin: false,
  baseUrl: "https://truyentuoitho.com/",
};

const SEARCH_ITEM = {
  sourceId: "madara-truyentuoitho-com",
  sourceMangaId: "ninja-loan-thi",
  sourceDisplayName: "Truyen Tuoi Tho",
  title: "Ninja Loạn Thị",
  author: null,
  coverKey: null,
  status: "ongoing",
};

function remoteDetail(savedMangaId: string | null) {
  return {
    ...SEARCH_ITEM,
    description: "Mô tả plain text.",
    genres: ["Shounen", "Comedy"],
    canonicalUrl: "https://truyentuoitho.com/manga/ninja-loan-thi/",
    chapters: [
      { sourceChapterId: "tap-1", title: "Chương 1", number: 1, order: 0, publishedAt: null },
      { sourceChapterId: "tap-2", title: "Tap 2", number: 2, order: 1, publishedAt: null },
    ],
    savedMangaId,
    availableOffline: false,
  };
}

type Backend = {
  sources: unknown[];
  saved: boolean;
  calls: Array<[string, Record<string, unknown> | undefined]>;
};

let backend: Backend;

beforeEach(() => {
  backend = { sources: [TTT_SOURCE], saved: false, calls: [] };
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    backend.calls.push([cmd, args]);
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" });
      case "app_health_check":
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 3 });
      case "list_supported_sources":
        return Promise.resolve(backend.sources);
      case "search_manga": {
        const query = String(args?.query ?? "");
        const errors =
          query.includes("lỗi") || query.includes("net")
            ? [{ sourceId: TTT_SOURCE.id, code: "SOURCE_NETWORK", message: "Không kết nối được nguồn (mạng/chặn theo khu vực)." }]
            : [];
        return Promise.resolve({
          items: query === "khongco" ? [] : [SEARCH_ITEM],
          sourceErrors: errors,
          hasMore: false,
        });
      }
      case "resolve_manga_url":
        return Promise.reject({
          code: "UNSUPPORTED_SOURCE",
          message: "URL không thuộc nguồn nào được hỗ trợ.",
          retryable: false,
        });
      case "get_remote_manga_details":
        return Promise.resolve(remoteDetail(backend.saved ? "m-77" : null));
      case "save_discovered_manga":
        backend.saved = true;
        return Promise.resolve({ mangaId: "m-77", savedChapterCount: 2, updated: false });
      case "add_manga_source": {
        backend.sources = [
          ...backend.sources,
          {
            id: "madara-vd-site-com",
            displayName: String(args?.displayName ?? ""),
            capabilities: ["search", "resolveUrl", "details"],
            status: "ready",
            attribution: null,
            enabled: true,
            builtin: false,
            baseUrl: String(args?.baseUrl ?? ""),
          },
        ];
        return Promise.resolve(backend.sources[1]);
      }
      case "update_manga_source": {
        backend.sources = backend.sources.map((source) =>
          source.id === args?.sourceId
            ? { ...source, displayName: String(args?.displayName), baseUrl: String(args?.baseUrl) }
            : source,
        );
        return Promise.resolve(backend.sources.find((source) => source.id === args?.sourceId));
      }
      case "set_source_enabled":
        return Promise.resolve(Boolean(args?.enabled));
      case "remove_manga_source":
        return Promise.resolve(undefined);
      case "get_home_feed":
        return Promise.resolve({ featured: [], newest: [], popular: [] });
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

describe("T05 live search", () => {
  it("shows attributed results alongside per-source errors instead of a fake empty state", async () => {
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/Nhập tên truyện/);
    fireEvent.change(input, { target: { value: "net" } });

    expect(await screen.findByText("Ninja Loạn Thị")).toBeDefined();
    expect(screen.getByText(/Nguồn: Truyen Tuoi Tho/)).toBeDefined();
    const errors = await screen.findByLabelText("Một số nguồn gặp lỗi");
    expect(within(errors).getByText("SOURCE_NETWORK")).toBeDefined();
    expect(within(errors).getByText(/Không kết nối được nguồn/)).toBeDefined();
    expect(screen.queryByText("Không có kết quả")).toBeNull();
  });

  it("links a result to the remote preview route", async () => {
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/Nhập tên truyện/);
    fireEvent.change(input, { target: { value: "ninja" } });
    const link = await screen.findByRole("link", { name: "Mở xem trước Ninja Loạn Thị" });
    expect(link.getAttribute("href")).toContain(
      "/remote/madara-truyentuoitho-com/ninja-loan-thi",
    );
  });

  it("renders hostile provider titles as inert text (T05-12)", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_supported_sources") {
        return Promise.resolve([TTT_SOURCE]);
      }
      if (cmd === "search_manga") {
        return Promise.resolve({
          items: [{ ...SEARCH_ITEM, title: "<img src=x onerror=alert(1)>" }],
          sourceErrors: [],
          hasMore: false,
        });
      }
      if (cmd === "get_home_feed") {
        return Promise.resolve({ featured: [], newest: [], popular: [] });
      }
      return Promise.resolve([]);
    });
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/Nhập tên truyện/);
    fireEvent.change(input, { target: { value: "doc" } });
    expect(await screen.findByText("<img src=x onerror=alert(1)>")).toBeDefined();
    expect(document.querySelector("img[src='x']")).toBeNull();
  });

  it("appends page 2 via 'Xem thêm' without losing page 1, then hides the button", async () => {
    const pages: Array<Record<string, unknown> | undefined> = [];
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "list_supported_sources") {
        return Promise.resolve([TTT_SOURCE]);
      }
      if (cmd === "search_manga") {
        pages.push(args);
        const page = Number(args?.page ?? 1);
        const item = (n: number) => ({
          ...SEARCH_ITEM,
          sourceMangaId: `m-${n}`,
          title: page === 2 && n === 2 ? "Trang hai" : `Truyện ${n}`,
        });
        return Promise.resolve(
          page === 1
            ? { items: [item(1)], sourceErrors: [], hasMore: true }
            : { items: [item(2)], sourceErrors: [], hasMore: false },
        );
      }
      if (cmd === "get_home_feed") {
        return Promise.resolve({ featured: [], newest: [], popular: [] });
      }
      return Promise.resolve([]);
    });
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/Nhập tên truyện/);
    fireEvent.change(input, { target: { value: "ninja" } });

    expect(await screen.findByText("Truyện 1")).toBeDefined();
    const more = await screen.findByRole("button", { name: "Xem thêm" });
    fireEvent.click(more);

    expect(await screen.findByText("Trang hai")).toBeDefined();
    // Page 1 stays visible while page 2 is appended below it.
    expect(screen.getByText("Truyện 1")).toBeDefined();
    // Exhausted hasMore hides the button; the second call carried page: 2.
    expect(screen.queryByRole("button", { name: "Xem thêm" })).toBeNull();
    expect(pages[0]).toEqual(expect.objectContaining({ page: 1 }));
    expect(pages[1]).toEqual(expect.objectContaining({ page: 2 }));
  });
});

describe("T05 URL import", () => {
  it("surfaces the structured rejection code without fetching unknown hosts", async () => {
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/thuộc nguồn đã đăng ký/);
    fireEvent.change(input, { target: { value: "https://not-registered.test/manga/x/" } });
    fireEvent.click(screen.getByRole("button", { name: "Phân tích" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("URL không thuộc nguồn nào được hỗ trợ.");
    expect(alert.textContent).toContain("UNSUPPORTED_SOURCE");
  });
});

describe("T05 remote preview and explicit save", () => {
  it("saves metadata only once and distinguishes saved from downloaded", async () => {
    renderAt("/remote/madara-truyentuoitho-com/ninja-loan-thi");
    expect(await screen.findByRole("heading", { name: "Ninja Loạn Thị" })).toBeDefined();
    expect(screen.getByText("Xem trước từ xa")).toBeDefined();
    expect(screen.getByText("Chưa tải — Phase 06")).toBeDefined();
    expect(screen.getByText("Chương (2)")).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: "Lưu vào Library" }));
    const status = await screen.findByRole("status");
    expect(status.textContent).toContain("Đã lưu vào Library (chỉ metadata) — 2 chương");
    expect(status.textContent).toContain("chỉ metadata");

    // After refetch the backend reports the saved internal id via savedMangaId.
    expect(await screen.findByText("Đã lưu metadata")).toBeDefined();
    expect(screen.getByRole("button", { name: "Làm mới metadata & chương" })).toBeDefined();
    const saveCall = backend.calls.find(([cmd]) => cmd === "save_discovered_manga");
    expect(saveCall?.[1]).toEqual({
      sourceId: "madara-truyentuoitho-com",
      sourceMangaId: "ninja-loan-thi",
    });
  });
});

describe("T05 sources management", () => {
  it("adds a user-registered source through IPC and refetches the registry", async () => {
    renderAt("/settings/sources");
    const nameInput = await screen.findByLabelText(/Tên nguồn mới/);
    const urlInput = screen.getByLabelText(/URL gốc/);
    fireEvent.change(nameInput, { target: { value: "VD Site" } });
    fireEvent.change(urlInput, { target: { value: "https://vd-site.com/" } });
    fireEvent.click(screen.getByRole("button", { name: "Thêm nguồn" }));

    expect(await screen.findByText("VD Site")).toBeDefined();
    const addCall = backend.calls.find(([cmd]) => cmd === "add_manga_source");
    expect(addCall?.[1]).toEqual({ displayName: "VD Site", baseUrl: "https://vd-site.com/" });
  });

  it("edits a built-in source domain through update_manga_source keeping its id", async () => {
    renderAt("/settings/sources");
    const heading = await screen.findByText("Truyen Tuoi Tho");
    const row = heading.closest(".source-row") as HTMLElement;
    fireEvent.click(row.querySelector("button") as Element);
    const domainInput = screen.getByLabelText(/Tên miền mới/);
    fireEvent.change(domainInput, { target: { value: "https://truyentuoitho.work/" } });
    fireEvent.click(screen.getByRole("button", { name: "Lưu tên miền" }));
    await waitFor(() => {
      const call = backend.calls.find(([cmd]) => cmd === "update_manga_source");
      expect(call?.[1]).toMatchObject({
        sourceId: "madara-truyentuoitho-com",
        baseUrl: "https://truyentuoitho.work/",
      });
    });
    expect(await screen.findByText("https://truyentuoitho.work/")).toBeDefined();
  });

  it("keeps the demo sandbox section separate from the real registry", async () => {
    renderAt("/settings/sources");
    // The sidebar carries its own "Nguồn truyện" nav label, so the page
    // heading has to be asserted inside the content region.
    const content = within(screen.getByLabelText("Nội dung ứng dụng"));
    expect(await content.findByText("Nguồn truyện")).toBeDefined();
    expect(screen.getByText("Demo sandbox (Phase 03)")).toBeDefined();
        // Demo mock sources render their own toggles, separate from real ones.
    const demoToggles = await screen.findAllByRole("button", { name: "Tắt nguồn" });
    expect(demoToggles.length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Tắt", hidden: false })).toBeDefined();
  });
});

describe("T05 Library truthfulness (round-1 finding)", () => {
  it("labels saved-but-not-downloaded manga as having no offline content", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "library_list") {
        return Promise.resolve({
          items: [
            {
              id: "m-1",
              title: "Ninja Loạn Thị",
              author: null,
              status: "ongoing",
              favorite: false,
              downloadedChapters: 0,
              totalChapters: 2,
              localSizeBytes: null,
              lastReadAt: null,
              lastDownloadAt: null,
              progress: null,
              hasMissing: false,
            },
          ],
          total: 1,
          limit: 24,
          offset: 0,
        });
      }
      if (cmd === "library_stats") {
        return Promise.resolve({
          savedMangas: 1,
          totalChapters: 2,
          downloadedChapters: 0,
          downloadedMangas: 0,
          localSizeBytes: 0,
          favorites: 0,
          reading: 0,
          completed: 0,
          paused: 0,
          missing: 0,
        });
      }
      if (cmd === "storage_status") {
        return Promise.resolve({
          libraryBytes: 0,
          cacheBytes: 0,
          stagingDirs: 0,
          activeTasks: 0,
          rootAvailable: true,
          approximate: true,
          customRoot: false,
          rootLinkOk: true,
          volumeTotalBytes: null,
          volumeFreeBytes: null,
        });
      }
      if (cmd === "library_genres") {
        return Promise.resolve([]);
      }
      if (cmd === "app_list_read_history") {
        return Promise.resolve({ items: [], total: 0, limit: 3, offset: 0 });
      }
      return Promise.resolve([]);
    });
    renderAt("/library");
    // Saved metadata with zero downloaded chapters must NOT look downloaded.
    // The same row can appear in more than one rail, so assert the label on
    // every place it is shown rather than assuming a single card.
    const labels = await screen.findAllByText("0/2 chương offline");
    expect(labels.length).toBeGreaterThan(0);
    expect(screen.getAllByText("Chưa tải").length).toBeGreaterThan(0);
    expect(screen.queryByText(/đã có file|chương offline \d/)).toBeNull();
  });
});

describe("T05 Discover Phase 1 panels", () => {
  const FEATURED = {
    ...SEARCH_ITEM,
    sourceId: "truyenqq",
    sourceMangaId: "vo-luyen-dinh-phong",
    sourceDisplayName: "TruyenQQ",
    title: "Võ Luyện Đỉnh Phong",
    status: "unknown",
    latestChapter: "Chapter 3824",
    latestTime: null,
  };

  it("saves a featured suggestion through the quick add button", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      backend.calls.push([cmd, args]);
      if (cmd === "get_home_feed") {
        return Promise.resolve({ featured: [FEATURED], newest: [], popular: [] });
      }
      if (cmd === "list_supported_sources") return Promise.resolve(backend.sources);
      if (cmd === "save_discovered_manga") {
        return Promise.resolve({ mangaId: "m-77", savedChapterCount: 2, updated: false });
      }
      return Promise.resolve({ items: [], sourceErrors: [], hasMore: false });
    });

    renderAt("/discover");
    expect(await screen.findByText("Võ Luyện Đỉnh Phong")).toBeDefined();
    // The rail is labelled for what it actually is, not as a taste model.
    expect(screen.getByText(/chưa có gợi ý theo sở thích/)).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: "Thêm Võ Luyện Đỉnh Phong vào thư viện" }));
    expect(await screen.findByText("Đã lưu 2 chương")).toBeDefined();
    const call = backend.calls.find(([cmd]) => cmd === "save_discovered_manga");
    expect(call?.[1]).toEqual({ sourceId: "truyenqq", sourceMangaId: "vo-luyen-dinh-phong" });
  });

  it("toggles a source from the supported-sources rail and links to the registry", async () => {
    renderAt("/discover");
    const box = await screen.findByRole("checkbox", { name: "Kích hoạt Truyen Tuoi Tho" });
    expect((box as HTMLInputElement).checked).toBe(true);
    fireEvent.click(box);
    await waitFor(() => {
      const call = backend.calls.find(([cmd]) => cmd === "set_source_enabled");
      expect(call?.[1]).toEqual({ sourceId: "madara-truyentuoitho-com", enabled: false });
    });
    expect(
      screen.getByRole("link", { name: /Quản lý nguồn truyện/ }).getAttribute("href"),
    ).toContain("/settings/sources");
  });
});

describe("T05 error taxonomy through normalizeIpcError", () => {
  it("passes through new source codes and keeps unknown ones INTERNAL_ERROR", () => {
    expect(normalizeIpcError({ code: "SOURCE_NETWORK", message: "x" }).code).toBe("SOURCE_NETWORK");
    expect(normalizeIpcError({ code: "INVALID_URL", message: "x" }).code).toBe("INVALID_URL");
    expect(normalizeIpcError({ code: "SOURCE_RATE_LIMITED", message: "x" }).code).toBe(
      "SOURCE_RATE_LIMITED",
    );
    expect(normalizeIpcError({ code: "NOT_A_CODE", message: "x" }).code).toBe("INTERNAL_ERROR");
  });
});
