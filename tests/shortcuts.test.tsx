/// The key map existed only in the reader's source; "?" now lists every
/// binding, and must not fire while the user is typing a query.
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "app_list_local_download_tasks") {
      return Promise.resolve({ items: [], total: 0, limit: 100, offset: 0 });
    }
    if (cmd === "get_download_queue_paused") return Promise.resolve(0);
    return Promise.resolve(null);
  });
});

function renderApp() {
  render(
    <MemoryRouter initialEntries={["/downloads"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("Bảng phím tắt", () => {
  it("opens on ? and lists the real global and reader bindings", async () => {
    renderApp();
    await screen.findByText(/Hàng đợi tải/);
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.keyDown(window, { key: "?" });

    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Ctrl + K");
    expect(dialog.textContent).toContain("Trang sau");
    expect(dialog.textContent).toContain("toàn màn hình");
  });

  it("stays shut while the user is typing a query", async () => {
    renderApp();
    const input = await screen.findByPlaceholderText(/Dán link chương/);

    fireEvent.keyDown(input, { key: "?" });

    expect(screen.queryByRole("dialog")).toBeNull();
  });
});