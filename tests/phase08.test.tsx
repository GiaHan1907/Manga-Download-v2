import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppRoutes } from "../src/app/router";
import {
  applyDownloadEventToInvalidation,
} from "../src/app/downloadInvalidation";
import { domainsForTaskStatus } from "../src/app/downloadInvalidation";
import {
  bumpDomain,
  useDomainVersion,
} from "../src/shared/hooks/useInvalidation";
import type { DownloadTaskEvent } from "../src/types/downloads";

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
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

import { invoke } from "@tauri-apps/api/core";

function taskEvent(status: string): DownloadTaskEvent {
  return {
    task: {
      id: "t1",
      chapterId: "c1",
      status,
      targetFormat: "cbz",
      priority: 0,
      completedPages: 1,
      totalPages: 2,
      attempts: 0,
      nextRetryAt: null,
      errorCode: null,
      errorMessage: null,
      revision: 3,
      finishedAt: null,
      outputRelpath: null,
      chapterTitle: "Chương 1",
      mangaTitle: "Truyện A",
      createdAt: "2026-09-23T00:00:00.000Z",
      updatedAt: "2026-09-23T00:00:00.000Z",
    },
    downloadedBytes: 0,
    speedBytesPerSecond: null,
    occurredAt: "2026-09-23T00:00:00.000Z",
  };
}

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "app_get_info":
        return Promise.resolve({ name: "x", version: "0.1.0", platform: "windows" });
      case "app_health_check":
        return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 5 });
      case "app_get_settings":
        return Promise.resolve({
          defaultReaderMode: "single",
          defaultReadingDirection: "rtl",
          chapterConcurrency: 2,
          imageConcurrency: 4,
          outputFormat: "cbz",
          notificationsEnabled: true,
          closeToTray: false,
        });
      case "library_stats":
        return Promise.resolve({ savedMangas: 1, downloadedChapters: 2, localSizeBytes: 3, favorites: 0 });
      case "app_list_local_download_tasks":
        return Promise.resolve({ items: [], total: 0, limit: 5, offset: 0 });
      case "library_list":
        return Promise.resolve({
          items: [
            {
              id: "m-9",
              title: "<img src=x onerror=alert(1)>Ninja Loạn Thị",
              author: null,
              status: "ongoing",
              favorite: false,
              downloadedChapters: 2,
              totalChapters: 5,
              localSizeBytes: 10,
              lastReadAt: null,
              lastDownloadAt: null,
              progress: null,
              hasMissing: false,
            },
          ],
          total: 1,
          limit: 6,
          offset: 0,
        });
      case "app_list_favorite_mangas":
        return Promise.resolve({
          items: [
            {
              id: "f1",
              sourceId: "madara-x",
              title: "Truyện Favor 1",
              author: null,
              status: "ongoing",
              chapterCount: 3,
              favorite: true,
              updatedAt: "2026-09-23T00:00:00.000Z",
            },
          ],
          total: 25,
          limit: 20,
          offset: 0,
        });
      case "app_set_manga_favorite":
        return Promise.resolve(true);
      case "storage_status":
        return Promise.resolve({
          libraryBytes: 2048,
          cacheBytes: 512,
          stagingDirs: 1,
          activeTasks: 0,
          rootAvailable: true,
          approximate: true,
          customRoot: false,
          rootLinkOk: true,
        });
      case "get_download_root":
        return Promise.resolve({
          isCustom: false,
          label: "Mặc định (AppData của app)",
          linkOk: true,
        });
      case "clear_regenerable_cache":
        return Promise.resolve({ removedDirs: 2, freedBytes: 4096, keptActiveDirs: 1 });
      default:
        return Promise.reject(new Error(`unexpected command ${cmd}`));
    }
  });
});

describe("T08 cross-screen invalidation (P08-02)", () => {
  it("maps task statuses to the domains that must refetch", () => {
    expect(domainsForTaskStatus("completed")).toEqual(["downloads", "library", "home"]);
    expect(domainsForTaskStatus("failed")).toEqual(["downloads", "home"]);
    // Progress frames (every ~250ms) must NOT trigger cross-screen refetch.
    expect(domainsForTaskStatus("downloading")).toEqual([]);
    expect(domainsForTaskStatus("queued")).toEqual([]);
    expect(domainsForTaskStatus("verifying")).toEqual([]);
    expect(domainsForTaskStatus("bogus")).toEqual([]);
  });

  it("useDomainVersion tracks bumps for the requested domain only", async () => {
    let libraryRenders = 0;
    function Probe() {
      const library = useDomainVersion("library");
      useDomainVersion("home");
      libraryRenders += 1;
      return (
        <span>
          L{library}
        </span>
      );
    }
    render(<Probe />);
    const before = libraryRenders;
    bumpDomain("favorites");
    await waitFor(() => expect(libraryRenders).toBe(before));
    bumpDomain("library");
    await waitFor(() => expect(screen.getByText(/^L1$/)).toBeDefined());
    applyDownloadEventToInvalidation(taskEvent("completed"));
    await waitFor(() => expect(screen.getByText(/^L2$/)).toBeDefined());
  });
});

describe("T08-10/T08-11/T08-12 command palette", () => {
  it("opens with Ctrl+K, searches ONLY locally, keyboard-selects and navigates", async () => {
    renderAt("/");
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    await screen.findByRole("dialog", { name: "Tìm kiếm và lệnh nhanh" });

    const input = screen.getByLabelText("Nhập từ khóa tìm kiếm cục bộ");
    fireEvent.change(input, { target: { value: "ninja" } });
    // Debounce must not fire any remote search command — only library_list.
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("library_list", expect.objectContaining({ query: "ninja" })));
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "search_manga")).toBe(false);

    const options = await screen.findAllByRole("option");
    const mangaOption = options[0];
    // XSS guard: HTML-ish title renders inert (no img element from metadata).
    expect(document.querySelector(".palette img")).toBeNull();
    expect(mangaOption.textContent).toContain("Ninja Loạn Thị");

    // Keyboard: Down moves the highlight, Up returns, Enter selects.
    fireEvent.keyDown(input, { key: "ArrowDown" });
    await waitFor(() =>
      expect(screen.getAllByRole("option")[1].getAttribute("aria-selected")).toBe("true"),
    );
    fireEvent.keyDown(input, { key: "ArrowUp" });
    await waitFor(() =>
      expect(screen.getAllByRole("option")[0].getAttribute("aria-selected")).toBe("true"),
    );
    fireEvent.keyDown(input, { key: "Enter" });
    // Navigated to the local detail of m-9 (unmocked command → its real
    // error state proves the route changed, and the dialog is gone).
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(await screen.findByText("Không mở được truyện đã lưu")).toBeDefined();

    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    await screen.findByRole("dialog");
    fireEvent.keyDown(screen.getByLabelText("Nhập từ khóa tìm kiếm cục bộ"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});

describe("T08-15/T08-16 favorites pagination + storage panel", () => {
  it("favorites page paginates against the backend", async () => {
    renderAt("/favorites");
    await screen.findByText("Truyện Favor 1");
    expect(screen.getByText(/tổng 25 bản ghi/i)).toBeDefined();
    vi.mocked(invoke).mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Sau" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("app_list_favorite_mangas", { limit: 20, offset: 20 }),
    );
  });

  it("settings storage panel shows sizes and clears only regenerable cache (inline confirm)", async () => {
    renderAt("/settings");
    // Panel is intentionally terse now: root line + actions (no stats prose).
    expect(await screen.findByText(/Nơi lưu truyện đã tải/)).toBeDefined();
    // Two-step inline confirmation — no native dialog in webviews.
    fireEvent.click(screen.getByRole("button", { name: "Dọn cache tái tạo được" }));
    expect(await screen.findByText(/Sẽ xóa staging của các tác vụ đã kết thúc/)).toBeDefined();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "clear_regenerable_cache")).toBe(
      false,
    );
    fireEvent.click(screen.getByRole("button", { name: "Xác nhận dọn cache tạm" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("clear_regenerable_cache"));
    expect(await screen.findByText(/Đã xóa 2 thư mục tạm/i)).toBeDefined();
  });

  it("relocates the download root via the Rust folder command and shows the new label", async () => {
    const calls: string[] = [];
    let relocated = false;
    const base = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      calls.push(cmd);
      if (cmd === "get_download_root") {
        return Promise.resolve(
          relocated
            ? { isCustom: true, label: "D:\\Manga", linkOk: true }
            : { isCustom: false, label: "Mặc định (AppData của app)", linkOk: true },
        );
      }
      if (cmd === "choose_download_root") {
        relocated = true;
        return Promise.resolve({ isCustom: true, label: "D:\\Manga", linkOk: true });
      }
      if (cmd === "storage_status") {
        return Promise.resolve({
          libraryBytes: 2048,
          cacheBytes: 512,
          stagingDirs: 0,
          activeTasks: 0,
          rootAvailable: true,
          approximate: true,
          customRoot: relocated,
          rootLinkOk: true,
        });
      }
      return (base as (c: string, a?: Record<string, unknown>) => Promise<unknown>)(cmd, args);
    });

    renderAt("/settings");
    expect(await screen.findByText(/Mặc định \(AppData của app\)/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Chọn thư mục lưu…/ }));
    await waitFor(() => expect(calls).toContain("choose_download_root"));
    expect(await screen.findByText("D:\\Manga")).toBeDefined();
    // After a successful move the "reset" path needs its own inline confirm.
    fireEvent.click(screen.getByRole("button", { name: "Về nơi lưu mặc định" }));
    expect(await screen.findByText(/Xác nhận: chuyển về mặc định/)).toBeDefined();
  });

  it("Settings exposes an entry link to the sources registry page", async () => {
    renderAt("/settings");
    fireEvent.click(await screen.findByRole("link", { name: "Quản lý nguồn (Sources)" }));
    expect(await screen.findByText("Nguồn truyện")).toBeDefined();
  });

  it("surfaces a broken custom link as an alert, never an empty library", async () => {    const base = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_download_root") {
        return Promise.resolve({ isCustom: true, label: "E:\\Manga", linkOk: false });
      }
      if (cmd === "storage_status") {
        return Promise.resolve({
          libraryBytes: 0,
          cacheBytes: 0,
          stagingDirs: 0,
          activeTasks: 0,
          rootAvailable: false,
          approximate: true,
          customRoot: true,
          rootLinkOk: false,
        });
      }
      return (base as (c: string, a?: Record<string, unknown>) => Promise<unknown>)(cmd, args);
    });
    renderAt("/settings");
    expect(await screen.findByText(/Link tới thư mục tùy chọn đang hỏng/)).toBeDefined();
  });
});
