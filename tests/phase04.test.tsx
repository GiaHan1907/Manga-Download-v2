import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "../src/app/router";
import Dialog from "../src/components/ui/Dialog";
import { normalizeIpcError } from "../src/shared/api/desktop";

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

let dbAvailable = true;

beforeEach(() => {
  dbAvailable = true;
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "app_get_info") {
      return Promise.resolve({ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" });
    }
    if (cmd === "app_health_check") {
      return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 2 });
    }
    if (cmd === "app_list_local_mangas" || cmd === "app_list_favorite_mangas") {
      if (!dbAvailable) {
        return Promise.reject({ code: "DATABASE_INIT_FAILED", message: "Không thể khởi tạo cơ sở dữ liệu local.", retryable: true });
      }
      return Promise.resolve({ items: [], total: 0, limit: 20, offset: 0 });
    }
    return Promise.reject(new Error(`Unknown command ${cmd}`));
  });
});

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("T04-09/T04-10 local library states", () => {
  it("distinguishes truthful empty from DB error", async () => {
    renderAt("/library");
    expect(await screen.findByText("Thư viện local đang trống")).toBeDefined();
    expect(screen.queryByRole("alert")).toBeNull();

    dbAvailable = false;
    // Navigate away and back to trigger a fresh fetch against the "failed" DB.
    fireEvent.click(screen.getByRole("link", { name: "Home" }));
    await screen.findByRole("link", { name: "Explore Manga" });
    fireEvent.click(screen.getByRole("link", { name: "Library" }));

    expect(await screen.findByText("Không đọc được Library từ database")).toBeDefined();
    expect(screen.getByText(/Không thể khởi tạo cơ sở dữ liệu local\./)).toBeDefined();
  });

  it("retry recovers when DB returns", async () => {
    renderAt("/library");
    await screen.findByText("Thư viện local đang trống");

    dbAvailable = false;
    fireEvent.click(screen.getByRole("link", { name: "Home" }));
    fireEvent.click(screen.getByRole("link", { name: "Library" }));
    const retry = await screen.findByRole("button", { name: "Thử lại" });

    dbAvailable = true;
    fireEvent.click(retry);
    expect(await screen.findByText("Thư viện local đang trống")).toBeDefined();
  });

  it("favorites page is local and truthful about the demo sandbox", async () => {
    renderAt("/favorites");
    expect(await screen.findByText("Chưa có mục yêu thích local")).toBeDefined();
    expect(screen.getByText(/khác với ★ demo ở Manga Detail/)).toBeDefined();
  });
});

describe("Dialog focus management (Phase 03 review carry-over)", () => {
  it("moves focus in, traps Tab and restores on unmount", async () => {
    const host = document.createElement("button");
    document.body.appendChild(host);
    host.focus();

    const onClose = vi.fn();
    const view = render(
      <Dialog open title="T" onClose={onClose}>
        <button type="button" onClick={onClose}>
          Đóng
        </button>
      </Dialog>,
    );
    const dialog = await screen.findByRole("dialog");
    await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));

    fireEvent.keyDown(window, { key: "Tab" });
    expect(dialog.contains(document.activeElement)).toBe(true);

    view.unmount();
    await waitFor(() => expect(document.activeElement).toBe(host));
    document.body.removeChild(host);
  });
});

describe("normalizeIpcError contract", () => {
  it("passes through Phase 04 codes and falls back safely", () => {
    expect(
      normalizeIpcError({ code: "VALIDATION_ERROR", message: "bad", retryable: false }).code,
    ).toBe("VALIDATION_ERROR");
    expect(normalizeIpcError({ code: "NOT_FOUND", message: "x", retryable: false }).code).toBe("NOT_FOUND");
    expect(
      normalizeIpcError({ code: "WEIRD_INTERNAL_THING", message: "x" }).code,
    ).toBe("INTERNAL_ERROR");
    expect(normalizeIpcError("plain string").message).toBe("plain string");
  });
});
