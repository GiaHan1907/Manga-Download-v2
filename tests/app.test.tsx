import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "../src/app/App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("App shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders loading then success state", async () => {
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

    render(<App />);

    expect(screen.getByText("Đang kiểm tra hệ thống...")).toBeDefined();

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
});
