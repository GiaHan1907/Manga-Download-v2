/// Export/import open native dialogs in Rust and answer with counts (or `null`
/// when the user cancels). The panel must say which of the two happened.
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

let exportAnswer: unknown;
let importAnswer: unknown;

beforeEach(() => {
  exportAnswer = { mangas: 12, favorites: 4, watched: 6, progress: 5, skipped: 0 };
  importAnswer = { mangas: 12, favorites: 3, watched: 2, progress: 4, skipped: 1 };
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "app_get_settings":
        return Promise.resolve(SETTINGS);
      case "app_list_recent_activity":
        return Promise.resolve([]);
      case "storage_status":
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
      case "get_download_root":
        return Promise.resolve({ isCustom: false, label: "Mặc định", path: null, linkOk: true });
      case "app_export_library_backup":
        return Promise.resolve(exportAnswer);
      case "app_import_library_backup":
        return Promise.resolve(importAnswer);
      default:
        return Promise.resolve(null);
    }
  });
});

function renderSettings() {
  render(
    <MemoryRouter initialEntries={["/settings"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("Sao lưu & khôi phục", () => {
  it("reports what the export wrote", async () => {
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "Xuất sao lưu" }));
    expect(
      await screen.findByText(
        "Đã lưu 12 truyện, 4 yêu thích, 6 đang theo dõi, 5 vị trí đọc vào file sao lưu.",
      ),
    ).toBeDefined();
  });

  it("reports a restore, including entries it had to skip", async () => {
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "Khôi phục từ file" }));
    expect(
      await screen.findByText(
        "Đã khôi phục 12 truyện, 3 yêu thích, 2 đang theo dõi, 4 vị trí đọc, bỏ qua 1 mục không có trong thư viện.",
      ),
    ).toBeDefined();
  });

  it("says so when the file dialog is cancelled instead of claiming success", async () => {
    exportAnswer = null;
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "Xuất sao lưu" }));
    await waitFor(() =>
      expect(screen.getByText("Đã huỷ — không có file nào được ghi hay đọc.")).toBeDefined(),
    );
  });
});