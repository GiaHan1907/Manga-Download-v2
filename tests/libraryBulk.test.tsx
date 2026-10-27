/// Bulk actions in the library: select a few stories, then remove them — with
/// or without their files — each call separate so one refusal (a manga with an
/// active download) does not abort the rest.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

import { invoke } from "@tauri-apps/api/core";

function manga(id: string, title: string) {
  return {
    id,
    title,
    description: null,
    author: null,
    status: "ongoing",
    genres: [],
    sourceId: "truyenqq",
    sourceMangaId: id,
    coverKey: null,
    favorite: false,
    downloadedChapters: 3,
    totalChapters: 10,
    localSizeBytes: 1024,
    lastReadAt: null,
    lastDownloadAt: null,
    progress: null,
    hasMissing: false,
    newAlerts: 0,
  };
}

let calls: Array<[string, Record<string, unknown> | undefined]>;

beforeEach(() => {
  calls = [];
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case "library_list":
        return Promise.resolve({
          items: [manga("m-1", "Ninja Loạn Thị"), manga("m-2", "Vua Pháp Thuật")],
          total: 2,
          limit: 24,
          offset: 0,
        });
      case "library_stats":
        return Promise.resolve({
          savedMangas: 2,
          totalChapters: 10,
          downloadedChapters: 6,
          downloadedMangas: 2,
          localSizeBytes: 2048,
          favorites: 0,
          reading: 0,
          completed: 0,
          paused: 0,
          missing: 0,
        });
      case "library_genres":
        return Promise.resolve([]);
      case "app_list_read_history":
        return Promise.resolve({ items: [], total: 0, limit: 5, offset: 0 });
      case "storage_status":
        return Promise.resolve({
          libraryBytes: 2048,
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
      case "library_remove_manga": {
        // The second manga has a live download and is refused, like Rust does.
        if (args?.mangaId === "m-2") {
          return Promise.reject({
            code: "VALIDATION_ERROR",
            message: "Truyện đang có tác vụ tải — dừng chúng trước khi gỡ.",
          });
        }
        return Promise.resolve({
          mangaId: args?.mangaId,
          chaptersRemoved: 3,
          filesRemoved: args?.deleteFiles ? 3 : 0,
          failedFiles: 0,
        });
      }
      default:
        return Promise.resolve(null);
    }
  });
});

function renderLibrary() {
  render(
    <MemoryRouter initialEntries={["/library"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("Thư viện: thao tác nhiều truyện", () => {
  it("removes the selected stories and keeps going after one refusal", async () => {
    renderLibrary();
    fireEvent.click(await screen.findByRole("button", { name: "Chọn nhiều" }));

    fireEvent.click(await screen.findByRole("checkbox", { name: "Chọn Ninja Loạn Thị" }));
    fireEvent.click(await screen.findByRole("checkbox", { name: "Chọn Vua Pháp Thuật" }));

    fireEvent.click(screen.getByRole("button", { name: "Gỡ khỏi Thư viện (giữ file)" }));

    await waitFor(() => expect(calls.filter(([cmd]) => cmd === "library_remove_manga")).toHaveLength(2));
    const payloads = calls
      .filter(([cmd]) => cmd === "library_remove_manga")
      .map(([, args]) => args);
    expect(payloads).toEqual([
      { mangaId: "m-1", deleteFiles: false },
      { mangaId: "m-2", deleteFiles: false },
    ]);
    expect(
      await screen.findByText(/Đã gỡ 1 truyện, 1 truyện không gỡ được/),
    ).toBeDefined();
  });

  it("ask before deleting files, and says how many it deleted", async () => {
    renderLibrary();
    fireEvent.click(await screen.findByRole("button", { name: "Chọn nhiều" }));
    fireEvent.click(await screen.findByRole("checkbox", { name: "Chọn Ninja Loạn Thị" }));
    fireEvent.click(screen.getByRole("button", { name: "Gỡ và xoá file" }));

    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("xoá file");
    // Nothing is sent until the destructive action is confirmed.
    expect(calls.some(([cmd]) => cmd === "library_remove_manga")).toBe(false);

    fireEvent.click(within(dialog).getByRole("button", { name: "Xác nhận xoá file" }));
    await waitFor(() => expect(calls.some(([cmd]) => cmd === "library_remove_manga")).toBe(true));
    expect(calls.find(([cmd]) => cmd === "library_remove_manga")?.[1]).toEqual({
      mangaId: "m-1",
      deleteFiles: true,
    });
    expect(await screen.findByText(/và xoá 3 file/)).toBeDefined();
  });

  it("select all on page covers the stories currently listed", async () => {
    renderLibrary();
    fireEvent.click(await screen.findByRole("button", { name: "Chọn nhiều" }));
    fireEvent.click(await screen.findByRole("button", { name: "Chọn cả trang" }));
    const bar = await screen.findByRole("region", { name: "Thao tác nhiều truyện" });
    expect(within(bar).getByText(/Đã chọn/)).toBeDefined();
    expect(within(bar).getByText("2")).toBeDefined();
  });
});