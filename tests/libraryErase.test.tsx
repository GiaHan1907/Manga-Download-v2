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

import { invoke } from "@tauri-apps/api/core";

let calls: Array<[string, Record<string, unknown> | undefined]>;

const MANGA = {
  id: "m-1",
  title: "Ninja Loạn Thị",
  description: null,
  author: null,
  status: "ongoing",
  genres: ["Hành động"],
  sourceId: "truyenqq",
  sourceMangaId: "ninja-loan-thi",
  coverKey: null,
  favorite: false,
  downloadedChapters: 4,
  totalChapters: 10,
  localSizeBytes: 1024,
  lastReadAt: null,
  lastDownloadAt: null,
  progress: null,
  hasMissing: false,
  newAlerts: 0,
};

beforeEach(() => {
  calls = [];
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({
          name: "Manga Downloader 1989",
          version: "1.0.0",
          platform: "windows",
        });
      case "library_list":
        return Promise.resolve({ items: [MANGA], total: 1, limit: 24, offset: 0 });
      case "library_genres":
        return Promise.resolve([{ genre: "Hành động", mangaCount: 1 }]);
      case "library_stats":
        return Promise.resolve({
          savedMangas: 1,
          totalChapters: 10,
          downloadedChapters: 4,
          downloadedMangas: 1,
          localSizeBytes: 1024,
          favorites: 0,
          reading: 0,
          completed: 0,
          paused: 0,
          missing: 0,
        });
      case "storage_status":
        return Promise.resolve({
          libraryBytes: 1024,
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
      case "app_list_read_history":
        return Promise.resolve({ items: [], total: 0, limit: 3, offset: 0 });
      case "library_remove_manga": {
        const deleteFiles = Boolean(args?.deleteFiles);
        return Promise.resolve({
          mangaId: "m-1",
          chaptersRemoved: 10,
          filesRemoved: deleteFiles ? 4 : 0,
          failedFiles: 0,
        });
      }
      default:
        return Promise.resolve(undefined);
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

async function openEraseDialog() {
  renderLibrary();
  // The page renders the same card in both layouts, so the trigger appears twice.
  const buttons = await screen.findAllByRole("button", {
    name: "Xóa Ninja Loạn Thị khỏi Thư viện",
  });
  fireEvent.click(buttons[0]);
  return screen.findByRole("dialog");
}

describe("Xóa truyện khỏi Thư viện — hai chế độ file", () => {
  it("mặc định GIỮ file trên đĩa và gửi deleteFiles=false", async () => {
    const dialog = await openEraseDialog();
    const keep = within(dialog).getByRole("radio", {
      name: /Giữ file đã tải/,
    }) as HTMLInputElement;
    expect(keep.checked).toBe(true);

    fireEvent.click(within(dialog).getByRole("button", { name: "Xóa truyện" }));
    await waitFor(() =>
      expect(calls.some(([cmd]) => cmd === "library_remove_manga")).toBe(true),
    );
    const call = calls.find(([cmd]) => cmd === "library_remove_manga");
    expect(call?.[1]).toEqual({ mangaId: "m-1", deleteFiles: false });
    // The toast must say the files survived — that is the promise the user asked for.
    expect(await screen.findByText(/file giữ nguyên/)).toBeDefined();
  });

  it("chọn xóa file thì gửi deleteFiles=true và không còn hứa giữ file", async () => {
    const dialog = await openEraseDialog();
    const remove = within(dialog).getByRole("radio", {
      name: /Xóa cả 4 file đã tải/,
    }) as HTMLInputElement;
    fireEvent.click(remove);
    expect(remove.checked).toBe(true);

    fireEvent.click(within(dialog).getByRole("button", { name: "Xóa truyện" }));
    await waitFor(() =>
      expect(calls.some(([cmd]) => cmd === "library_remove_manga")).toBe(true),
    );
    const call = calls.find(([cmd]) => cmd === "library_remove_manga");
    expect(call?.[1]).toEqual({ mangaId: "m-1", deleteFiles: true });
    expect(await screen.findByText(/xóa 4 file/)).toBeDefined();
  });
});
