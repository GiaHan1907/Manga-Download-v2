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
      return Promise.resolve({ status: "ok", database: "ready" });
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
      expect(screen.getByText("0.1.0")).toBeDefined();
    });

    expect(screen.getByText("windows")).toBeDefined();
    expect(screen.getByText("ready")).toBeDefined();
  });

  it("renders error state when invoke rejects", async () => {
    vi.mocked(invoke).mockRejectedValue({
      code: "INTERNAL_ERROR",
      message: "Test error",
      retryable: false,
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Lỗi: INTERNAL_ERROR")).toBeDefined();
    });

    expect(screen.getByText("Test error")).toBeDefined();
  });

  it("renders the downloads screen and highlights active sidebar item", async () => {
    mockHealthySystem();

    render(
      <MemoryRouter initialEntries={["/downloads"]}>
        <AppRoutes />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Downloads" })).toBeDefined();
    expect(screen.getByRole("link", { name: "Downloads" }).className).toContain("sidebar__link--active");
    await waitFor(() => {
      expect(screen.getByText(/Hàng đợi demo —/)).toBeDefined();
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
