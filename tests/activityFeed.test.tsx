/// The settings screen carries the only durable record of what happened while
/// the user was elsewhere: a watcher alert, a finished download, a failure.
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
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

import { invoke } from "@tauri-apps/api/core";

const SETTINGS = {
  defaultReaderMode: "single",
  defaultReadingDirection: "rtl",
  chapterConcurrency: 2,
  imageConcurrency: 6,
  outputFormat: "cbz",
  notificationsEnabled: true,
  closeToTray: true,
  watchEnabled: true,
};

const FEED = [
  {
    kind: "alert",
    mangaId: "m-1",
    mangaTitle: "Ninja Loạn Thị",
    chapterTitle: "Chương 92",
    at: new Date(Date.now() - 3600_000).toISOString(),
    code: null,
  },
  {
    kind: "downloaded",
    mangaId: "m-2",
    mangaTitle: "Vua Pháp Thuật",
    chapterTitle: "Chương 9",
    at: new Date(Date.now() - 7200_000).toISOString(),
    code: null,
  },
  {
    kind: "failed",
    mangaId: "m-2",
    mangaTitle: "Vua Pháp Thuật",
    chapterTitle: "Chương 10",
    at: new Date(Date.now() - 10800_000).toISOString(),
    code: "SOURCE_NOT_FOUND",
  },
];

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "app_get_settings":
        return Promise.resolve(SETTINGS);
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
      case "get_download_root":
        return Promise.resolve({ isCustom: false, label: "Mặc định (app data)", path: null, linkOk: true });
      case "app_list_recent_activity":
        return Promise.resolve(FEED);
      case "app_open_log_dir":
        return Promise.resolve(null);
      default:
        return Promise.resolve(null);
    }
  });
});

describe("Nhật ký hoạt động", () => {
  it("lists alerts and download outcomes newest first, with the failure code", async () => {
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <AppRoutes />
      </MemoryRouter>,
    );

    const rows = await screen.findAllByRole("listitem");
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("Chương mới"),
      expect.stringContaining("Đã tải xong"),
      expect.stringContaining("Tải lỗi"),
    ]);
    expect(screen.getByText("Chương 92")).toBeDefined();
    expect(screen.getByText("SOURCE_NOT_FOUND")).toBeDefined();
    // Each row links back to its manga so the feed is actionable.
    expect(screen.getByRole("link", { name: "Ninja Loạn Thị" })).toBeDefined();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("app_list_recent_activity", { limit: 12 }),
    );
  });

  it("opens the log folder from the same panel", async () => {
    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <AppRoutes />
      </MemoryRouter>,
    );
    fireEvent.click(await screen.findByRole("button", { name: /Mở thư mục log/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("app_open_log_dir"));
  });
});
