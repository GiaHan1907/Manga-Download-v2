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

const GIB = 1024 ** 3;

const manga = (id: string, title: string, overrides: Record<string, unknown> = {}) => ({
  id,
  title,
  description: null,
  author: null,
  status: "ongoing",
  genres: ["Hành động"],
  sourceId: "truyenqq",
  sourceMangaId: id,
  coverKey: null,
  favorite: false,
  downloadedChapters: 4,
  totalChapters: 10,
  localSizeBytes: 2 * GIB,
  lastReadAt: new Date(Date.now() - 3_600_000).toISOString(),
  lastDownloadAt: new Date(Date.now() - 7_200_000).toISOString(),
  progress: { chapterId: `${id}-c`, pageIndex: 5, pageTotal: 20, updatedAt: "now" },
  hasMissing: false,
  newAlerts: 0,
  ...overrides,
});

const STATS = {
  savedMangas: 12,
  totalChapters: 240,
  downloadedChapters: 96,
  downloadedMangas: 9,
  localSizeBytes: 6 * GIB,
  favorites: 3,
  reading: 5,
  completed: 4,
  paused: 2,
  missing: 1,
};

let calls: Array<[string, Record<string, unknown> | undefined]>;

beforeEach(() => {
  calls = [];
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({ name: "Manga Downloader 1989", version: "1.0.0", platform: "windows" });
      case "library_list": {
        const filter = String(args?.filter ?? "all");
        const items =
          filter === "downloaded"
            ? [manga("m-1", "Truyện Đã Tải")]
            : [manga("m-1", "Truyện Đã Tải"), manga("m-2", "Truyện Chưa Tải", { downloadedChapters: 0, progress: null })];
        return Promise.resolve({ items, total: items.length, limit: 24, offset: 0 });
      }
      case "library_genres":
        return Promise.resolve([
          { genre: "Hành động", mangaCount: 7 },
          { genre: "Lãng mạn", mangaCount: 2 },
        ]);
      case "library_stats":
        return Promise.resolve(STATS);
      case "storage_status":
        return Promise.resolve({
          libraryBytes: STATS.localSizeBytes,
          cacheBytes: 0,
          stagingDirs: 0,
          activeTasks: 0,
          rootAvailable: true,
          approximate: true,
          customRoot: false,
          rootLinkOk: true,
          volumeTotalBytes: 500 * GIB,
          volumeFreeBytes: 254 * GIB,
        });
      case "app_list_read_history":
        return Promise.resolve({ items: [], total: 0, limit: 3, offset: 0 });
      default:
        return Promise.resolve(undefined);
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

describe("Library redesign — real numbers only", () => {
  it("sends the chosen genre and status to the database instead of filtering client-side", async () => {
    renderAt("/library");
    const genreSelect = await screen.findByLabelText(/Thể loại/);
    fireEvent.change(genreSelect, { target: { value: "Hành động" } });
    await vi.waitFor(() => {
      const call = calls.filter(([cmd]) => cmd === "library_list").at(-1);
      expect(call?.[1]).toMatchObject({ genre: "Hành động" });
    });

    const statusSelect = screen.getByLabelText(/Trạng thái/);
    fireEvent.change(statusSelect, { target: { value: "completed" } });
    await vi.waitFor(() => {
      const call = calls.filter(([cmd]) => cmd === "library_list").at(-1);
      expect(call?.[1]).toMatchObject({ genre: "Hành động", filter: "completed" });
    });
  });

  it("offers the source's own genres with our own story counts", async () => {
    renderAt("/library");
    const select = await screen.findByLabelText(/Thể loại/);
    expect(within(select).getByRole("option", { name: "Hành động (7)" })).toBeDefined();
    expect(within(select).getByRole("option", { name: "Lãng mạn (2)" })).toBeDefined();
  });

  it("a quick filter tile drives the same filter the pill uses", async () => {
    renderAt("/library");
    fireEvent.click(await screen.findByRole("button", { name: /Đang đọc/ }));
    await vi.waitFor(() => {
      const call = calls.filter(([cmd]) => cmd === "library_list").at(-1);
      expect(call?.[1]).toMatchObject({ filter: "reading" });
    });
  });

  it("shows used of total capacity when the volume is readable", async () => {
    renderAt("/library");
    expect(await screen.findByText(/Đã dùng 6\.0 GB \/ 500\.0 GB · 1%/)).toBeDefined();
  });

  it("says it cannot read the disk instead of inventing a quota", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "storage_status") {
        return Promise.resolve({
          libraryBytes: 6 * GIB,
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
      if (cmd === "library_list") {
        return Promise.resolve({ items: [manga("m-1", "Truyện Đã Tải")], total: 1, limit: 24, offset: 0 });
      }
      if (cmd === "library_stats") {
        return Promise.resolve(STATS);
      }
      if (cmd === "library_genres") {
        return Promise.resolve([]);
      }
      if (cmd === "app_list_read_history") {
        return Promise.resolve({ items: [], total: 0, limit: 3, offset: 0 });
      }
      return Promise.resolve(undefined);
    });
    renderAt("/library");
    expect(await screen.findByText(/Không đọc được dung lượng ổ đĩa/)).toBeDefined();
    expect(screen.queryByText(/\/ 500\.0 GB/)).toBeNull();
  });

  it("never shows a star rating, because no source publishes one", async () => {
    renderAt("/library");
    expect((await screen.findAllByText("Truyện Đã Tải")).length).toBeGreaterThan(0);
    expect(screen.queryByText(/★/)).toBeNull();
  });
});
