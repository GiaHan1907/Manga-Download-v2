/// "Kiểm tra" answers "is this source answering right now?" without leaving the
/// screen: the row keeps the latency/result count, or the source's own error
/// code, after the check.
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

const SOURCE = {
  id: "truyenqq",
  displayName: "TruyenQQ",
  capabilities: ["search", "resolveUrl", "details", "pages"],
  status: "ready",
  attribution: null,
  enabled: true,
  builtin: true,
  baseUrl: "https://truyenqqko.com/",
  imageHosts: null,
};

let probeResult: unknown;

beforeEach(() => {
  probeResult = {
    ok: true,
    latencyMs: 812,
    resultCount: 12,
    code: null,
    message: null,
  };
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "list_supported_sources":
        return Promise.resolve([SOURCE]);
      case "probe_manga_source":
        return Promise.resolve(probeResult);
      case "app_get_info":
        return Promise.resolve({ name: "x", version: "1.0.0", platform: "windows" });
      case "app_health_check":
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 11 });
      default:
        return Promise.resolve(null);
    }
  });
});

function renderSources() {
  render(
    <MemoryRouter initialEntries={["/settings/sources"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("Kiểm tra nguồn", () => {
  it("shows latency and result count on the row", async () => {
    renderSources();
    fireEvent.click(await screen.findByRole("button", { name: "Kiểm tra" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("probe_manga_source", { sourceId: "truyenqq" }),
    );
    expect(
      await screen.findByText("Nguồn trả lời OK · 812 ms · 12 kết quả cho “one piece”"),
    ).toBeDefined();
  });

  it("shows the source's own code when the source answers with a failure", async () => {
    probeResult = {
      ok: false,
      latencyMs: 240,
      resultCount: 0,
      code: "SOURCE_NOT_FOUND",
      message: "Không tìm thấy nội dung trên nguồn (có thể link đã hết hạn).",
    };
    renderSources();
    fireEvent.click(await screen.findByRole("button", { name: "Kiểm tra" }));
    expect(
      await screen.findByText(
        "SOURCE_NOT_FOUND: Không tìm thấy nội dung trên nguồn (có thể link đã hết hạn).",
      ),
    ).toBeDefined();
  });

  it("points at the base URL when the failure looks like a moved or blocking host", async () => {
    probeResult = {
      ok: false,
      latencyMs: 30_000,
      resultCount: 0,
      code: "SOURCE_NETWORK",
      message: "Không kết nối được nguồn (mạng/chặn theo khu vực).",
    };
    renderSources();
    fireEvent.click(await screen.findByRole("button", { name: "Kiểm tra" }));
    expect(await screen.findByText(/Kiểm tra này không tự đoán tên miền mới/)).toBeDefined();
    // The escape hatch is the existing edit form, not an invented suggestion.
    fireEvent.click(screen.getByRole("button", { name: "Sửa tên miền" }));
    expect(await screen.findByLabelText(/Tên miền mới/)).toBeDefined();
  });
});
