/// Settings → Cập nhật: reports the running version, offers the install action
/// only when a newer release exists, and never claims success on failure.
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
  speedLimitKbps: 0,
};

let checkAnswer: unknown;
let checkFails = false;

beforeEach(() => {
  checkAnswer = { current: "1.0.1", available: false, version: null, notes: null };
  checkFails = false;
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
      case "app_check_update":
        return checkFails
          ? Promise.reject({
              code: "VALIDATION_ERROR",
              message: "Không kiểm tra được bản cập nhật (mạng hoặc GitHub).",
            })
          : Promise.resolve(checkAnswer);
      case "app_install_update":
        return Promise.resolve(null);
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

describe("Cập nhật ứng dụng", () => {
  it("says the app is current and offers no install button", async () => {
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "Kiểm tra cập nhật" }));
    expect(
      await screen.findByText("Bạn đang dùng bản mới nhất (1.0.1)."),
    ).toBeDefined();
    expect(screen.queryByRole("button", { name: /Cài v/ })).toBeNull();
  });

  it("offers the install action for a newer version and sends it", async () => {
    checkAnswer = {
      current: "1.0.1",
      available: true,
      version: "1.0.2",
      notes: "Sửa lỗi tải ảnh PNG.",
    };
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "Kiểm tra cập nhật" }));

    const install = await screen.findByRole("button", { name: "Cài v1.0.2 & khởi động lại" });
    expect(screen.getByText("Sửa lỗi tải ảnh PNG.")).toBeDefined();
    fireEvent.click(install);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("app_install_update"));
  });

  it("keeps the backend's failure message instead of pretending success", async () => {
    checkFails = true;
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "Kiểm tra cập nhật" }));
    expect(
      await screen.findByText("Không kiểm tra được bản cập nhật (mạng hoặc GitHub)."),
    ).toBeDefined();
  });
});