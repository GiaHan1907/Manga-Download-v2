import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "../src/app/App";
import { AppRoutes } from "../src/app/router";
import Dialog from "../src/components/ui/Dialog";

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

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

import { invoke } from "@tauri-apps/api/core";

function mockHealthySystem() {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "app_get_info") {
      return Promise.resolve({
        name: "Manga Downloader 1989",
        version: "0.1.0",
        platform: "windows",
      });
    }
    if (cmd === "app_health_check") {
      return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 2 });
    }
    if (cmd === "app_list_local_download_tasks") {
      return Promise.resolve({ items: [], total: 0, limit: 100, offset: 0 });
    }
    if (cmd === "library_stats") {
      return Promise.resolve({
        savedMangas: 0,
        downloadedChapters: 0,
        localSizeBytes: 0,
        favorites: 0,
      });
    }
    if (cmd === "library_list") {
      return Promise.resolve({ items: [], total: 0, limit: 3, offset: 0 });
    }
    if (cmd === "get_home_feed") {
      return Promise.resolve({ featured: [], newest: [], popular: [] });
    }
    return Promise.reject(new Error("Unknown command"));
  });
}

describe("App shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.location.hash = "#/";
  });

  it("renders loading then success state", async () => {
    mockHealthySystem();

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("link", { name: "Trang chủ" })).toBeDefined();
    });

    expect(screen.getByText("Manga Downloader 1989")).toBeDefined();
  });

  it("renders error state when invoke rejects", async () => {
    vi.mocked(invoke).mockRejectedValue({
      code: "INTERNAL_ERROR",
      message: "Test error",
      retryable: false,
    });

    render(<App />);

    // Home's authoritative front-page query fails → real error state (no
    // silent fallback to demo numbers), with the sanitized IPC message.
    await waitFor(() => {
      expect(screen.getByText("Không lấy được bảng tin trang chủ")).toBeDefined();
    });
    expect(screen.getByText(/Test error/)).toBeDefined();
  });

  it("renders the downloads screen and highlights active sidebar item", async () => {
    mockHealthySystem();

    render(
      <MemoryRouter initialEntries={["/downloads"]}>
        <AppRoutes />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Tải xuống" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Tải xuống" }).className).toContain("sidebar__link--active");
    await waitFor(() => {
      expect(screen.getByText(/Hàng đợi tải —/)).toBeDefined();
    });
  });

  it("collapses and expands the sidebar", () => {
    mockHealthySystem();

    render(
      <MemoryRouter>
        <AppRoutes />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Thu gọn sidebar" }));
    expect(screen.getByRole("button", { name: "Mở rộng sidebar" })).toBeDefined();
  });

  it("renders a 404 route for unknown paths", () => {
    mockHealthySystem();

    render(
      <MemoryRouter initialEntries={["/missing"]}>
        <AppRoutes />
      </MemoryRouter>,
    );

    expect(screen.getByText("Không tìm thấy trang")).toBeDefined();
  });
});

describe("Dialog", () => {
  it("closes with Escape", () => {
    const onClose = vi.fn();

    render(
      <Dialog open title="Kiểm tra dialog" onClose={onClose}>
        Nội dung dialog
      </Dialog>,
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
