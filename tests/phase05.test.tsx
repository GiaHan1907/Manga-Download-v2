import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
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
      case "set_source_enabled":
        return Promise.resolve(Boolean(args?.enabled));
      case "remove_manga_source":
        return Promise.resolve(undefined);
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
    const input = await screen.findByPlaceholderText(/tối thiểu 2 ký tự/);
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
    const input = await screen.findByPlaceholderText(/tối thiểu 2 ký tự/);
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
      return Promise.resolve([]);
    });
    renderAt("/discover");
    const input = await screen.findByPlaceholderText(/tối thiểu 2 ký tự/);
    fireEvent.change(input, { target: { value: "doc" } });
    expect(await screen.findByText("<img src=x onerror=alert(1)>")).toBeDefined();
    expect(document.querySelector("img[src='x']")).toBeNull();
  });
});

describe("T05 URL import", () => {
  it("surfaces the structured rejection code without fetching unknown hosts", async () => {
    renderAt("/discover?tab=url");
    const input = await screen.findByPlaceholderText(/thuộc nguồn đã đăng ký/);
    fireEvent.change(input, { target: { value: "https://not-registered.test/manga/x/" } });
    fireEvent.click(screen.getByRole("button", { name: "Kiểm tra URL (thật)" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("URL không thuộc nguồn nào được hỗ trợ.");
    expect(alert.textContent).toContain("UNSUPPORTED_SOURCE");
  });
});

describe("T05 remote preview and explicit save", () => {
  it("saves metadata only once and distinguishes saved from downloaded", async () => {
    renderAt("/remote/madara-truyentuoitho-com/ninja-loan-thi");
    expect(await screen.findByRole("heading", { name: "Ninja Loạn Thị" })).toBeDefined();
    expect(screen.getByText("Remote preview")).toBeDefined();
    expect(screen.getByText("Chưa tải — Phase 06")).toBeDefined();
    expect(screen.getByText("Chương (2)")).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: "Lưu vào Library" }));
    const status = await screen.findByRole("status");
    expect(status.textContent).toContain("Đã lưu vào Library (chỉ metadata) — 2 chương");
    expect(status.textContent).toContain("Chưa tải file nào");

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

  it("keeps the demo sandbox section separate from the real registry", async () => {
    renderAt("/settings/sources");
    expect(await screen.findByText("Nguồn thật (Rust registry)")).toBeDefined();
    expect(screen.getByText("Demo sandbox (Phase 03)")).toBeDefined();
    expect(screen.getByText(/trạng thái mock của phiên demo/)).toBeDefined();
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
      return Promise.resolve([]);
    });
    renderAt("/library");
    // Saved metadata with zero downloaded chapters must NOT look downloaded.
    expect(await screen.findByText("Chưa tải chương nào")).toBeDefined();
    expect(screen.getByText("0/2 chương offline · không rõ tác giả")).toBeDefined();
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
