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
    if (
      cmd === "app_list_local_mangas" ||
      cmd === "app_list_favorite_mangas" ||
      cmd === "library_list"
    ) {
      if (!dbAvailable) {
        return Promise.reject({ code: "DATABASE_INIT_FAILED", message: "Không thể khởi tạo cơ sở dữ liệu local.", retryable: true });
      }
      return Promise.resolve({ items: [], total: 0, limit: 24, offset: 0 });
    }
    if (cmd === "library_get_downloaded_chapters") {
      return Promise.resolve({ chapters: [] });
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

describe("Favorites local flow (visible behavior, round 3)", () => {
  it("removes a persisted favorite through IPC and refetches to empty", async () => {
    const manga = {
      id: "m-9",
      sourceId: "src-a",
      sourceMangaId: "a9",
      title: "Truyện Local Đã Sao",
      author: null,
      description: null,
      status: "ongoing",
      chapterCount: 2,
      favorite: true,
      updatedAt: "2026-09-22T00:00:00.000Z",
    };
    const calls: Array<[string, unknown]> = [];
    vi.mocked(invoke).mockImplementation((cmd: string, args?: unknown) => {
      calls.push([cmd, args]);
      if (cmd === "app_list_favorite_mangas") {
        const alreadyRemoved = calls.some(([c, a]) => c === "app_set_manga_favorite" && (a as { favorite: boolean }).favorite === false);
        return Promise.resolve({ items: alreadyRemoved ? [] : [manga], total: alreadyRemoved ? 0 : 1, limit: 20, offset: 0 });
      }
      if (cmd === "app_set_manga_favorite") return Promise.resolve(false);
      return Promise.reject(new Error(`Unknown command ${cmd}`));
    });

    renderAt("/favorites");
    const remove = await screen.findByRole("button", { name: "Bỏ yêu thích" });
    fireEvent.click(remove);
    expect(await screen.findByText("Chưa có mục yêu thích local")).toBeDefined();
    expect(calls.some(([c]) => c === "app_set_manga_favorite")).toBe(true);
  });
});

describe("S1/S2 review-round-2 boundaries", () => {
  it("Manga Detail favorite is explicitly labeled demo with boundary note (§2.8)", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "app_get_info") {
        return Promise.resolve({ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" });
      }
      if (cmd === "app_health_check") {
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 2 });
      }
      return Promise.reject(new Error("Unknown command"));
    });

    renderAt("/manga/manga-001");
    const star = await screen.findByRole("button", { name: /êu thích \(demo\)/ });
    expect(star).toBeDefined();
    expect(await screen.findByText(/tab Favorites chỉ hiển thị dữ liệu local thật/)).toBeDefined();
  });

  it("Library favorite surfaces IPC failure instead of silent success (§P04-06)", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "app_get_info") {
        return Promise.resolve({ name: "Manga Downloader 1989", version: "0.1.0", platform: "windows" });
      }
      if (cmd === "library_list") {
        return Promise.resolve({
          items: [
            {
              id: "m-1",
              title: "Manga Đã Đăng Ký",
              author: null,
              status: "ongoing",
              favorite: false,
              downloadedChapters: 0,
              totalChapters: 3,
              localSizeBytes: null,
              lastReadAt: null,
              lastDownloadAt: null,
              progress: null,
              hasMissing: false,
            },
          ],
          total: 1,
          limit: 24,
          offset: 0,
        });
      }
      if (cmd === "app_set_manga_favorite") {
        return Promise.reject({ code: "NOT_FOUND", message: "Bản ghi không tồn tại.", retryable: false });
      }
      return Promise.reject(new Error("Unknown command"));
    });

    renderAt("/library");
    const star = await screen.findByRole("button", { name: "Yêu thích Manga Đã Đăng Ký" });
    fireEvent.click(star);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Bản ghi không tồn tại.");
    expect(await screen.findByText("Manga Đã Đăng Ký")).toBeDefined();
  });
});
