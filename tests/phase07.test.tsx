import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

import { invoke } from "@tauri-apps/api/core";

const manifest = {
  chapterId: "ch-1",
  mangaId: "mg-1",
  mangaTitle: "Hành Trình Offline",
  chapterTitle: "Chương 1",
  format: "cbz",
  pageCount: 3,
  pages: [
    { index: 0, width: 40, height: 60 },
    { index: 1, width: 40, height: 60 },
    { index: 2, width: 40, height: 60 },
  ],
  prevChapterId: null,
  nextChapterId: "ch-2",
  startPage: 0,
  startScrollOffset: null,
};

function pngBytes(): Uint8Array {
  return new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x1a, 0x0a, 0x00, 1, 2, 3]);
}

function mockCommands(cmd: string): Promise<unknown> {
  switch (cmd) {
    case "app_get_info":
      return Promise.resolve({ name: "x", version: "0.1.0", platform: "windows" });
    case "app_get_settings":
      return Promise.resolve({
        defaultReaderMode: "single",
        defaultReadingDirection: "rtl",
        chapterConcurrency: 2,
        imageConcurrency: 4,
        outputFormat: "cbz",
      });
    case "reader_open_chapter":
      return Promise.resolve(manifest);
    case "reader_get_page_bytes":
      return Promise.resolve(pngBytes());
    case "app_save_reading_progress":
      return Promise.resolve({
        mangaId: "mg-1",
        chapterId: "ch-1",
        pageIndex: 0,
        scrollOffset: null,
        pageTotal: 3,
        updatedAt: "2026-09-22T00:00:00.000Z",
      });
    default:
      return Promise.reject(new Error(`unexpected command ${cmd}`));
  }
}

function mockBackend() {
  vi.mocked(invoke).mockImplementation((cmd: string) => mockCommands(cmd));
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
  mockBackend();
});

describe("T07 offline reader UI (single mode, real manifest)", () => {
  it("renders the page, navigates forward/back and shows chapter nav", async () => {
    renderAt("/read/ch-1");
    const shell = await screen.findByTestId("reader-single");
    expect(await screen.findByText("1/3")).toBeDefined();
    // Prefetch window loaded current+next as blob URLs.
    await waitFor(() => {
      expect(within(shell).getAllByRole("img").length).toBeGreaterThan(0);
    });

    fireEvent.click(screen.getByRole("button", { name: "Trang sau" }));
    expect(await screen.findByText("2/3")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Trang trước" }));
    expect(await screen.findByText("1/3")).toBeDefined();

    // Keyboard: in RTL ArrowLeft advances further into the story.
    fireEvent.keyDown(window, { key: "ArrowLeft" });
    expect(await screen.findByText("2/3")).toBeDefined();
    // Chapter navigation only offers the existing next chapter.
    expect(screen.getByRole("button", { name: "Chương tiếp" })).toBeDefined();
    expect(screen.queryByRole("button", { name: "Chương trước" })).toBeNull();
    // No storage detail ever reaches the DOM — only opaque blob: URLs.
    expect(document.body.innerHTML).not.toContain(".cbz");
  });

  it("mode buttons switch layouts; double mode pairs pages with RTL order", async () => {
    renderAt("/read/ch-1");
    await screen.findByTestId("reader-single");
    fireEvent.click(screen.getByRole("button", { name: "Hai trang" }));
    const double = await screen.findByTestId("reader-double");
    expect(double.getAttribute("dir")).toBe("rtl");
    fireEvent.click(screen.getByRole("button", { name: "Cuộn dọc" }));
    expect(await screen.findByTestId("reader-vertical")).toBeDefined();
    // Mode switch keeps the logical page.
    expect(screen.getByText("1/3")).toBeDefined();
  });

  it("restores the resume position and resets the page when switching chapters", async () => {
    const ch2 = {
      ...manifest,
      chapterId: "ch-2",
      chapterTitle: "Chương 2",
      pageCount: 5,
      pages: [0, 1, 2, 3, 4].map((index) => ({ index, width: 40, height: 60 })),
      prevChapterId: "ch-1",
      nextChapterId: null,
      startPage: 0,
    };
    vi.mocked(invoke).mockImplementation((cmd: string, payload?: unknown) => {
      if (cmd === "reader_open_chapter") {
        const chapterId = (payload as { chapterId?: string } | undefined)?.chapterId;
        return Promise.resolve(chapterId === "ch-2" ? ch2 : { ...manifest, startPage: 2 });
      }
      return mockCommands(cmd);
    });
    renderAt("/read/ch-1");
    // ch-1 stores progress at page index 2 → resumes on "3/3", not page 1.
    expect(await screen.findByText("3/3")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Chương tiếp" }));
    // Same component, new chapter: stale page 3 must NOT leak into a 5-page
    // chapter that has its own (empty) resume position.
    expect(await screen.findByText("1/5")).toBeDefined();
    expect(screen.getByRole("button", { name: "Chương trước" })).toBeDefined();
  });

  it("controlled error when the artifact cannot be opened", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "reader_open_chapter") {
        throw {
          code: "LOCAL_FILE_MISSING",
          message: "File đã mất khỏi đĩa — cần tải lại chương này.",
          retryable: false,
        };
      }
      if (cmd === "app_get_settings") {
        return {
          defaultReaderMode: "single",
          defaultReadingDirection: "ltr",
          chapterConcurrency: 2,
          imageConcurrency: 4,
          outputFormat: "cbz",
        };
      }
      return { name: "x", version: "0.1.0", platform: "windows" };
    });
    renderAt("/read/ch-1");
    expect(await screen.findByText("Không mở được chương")).toBeDefined();
    expect(screen.getByText(/File đã mất khỏi đĩa/)).toBeDefined();
  });
});
