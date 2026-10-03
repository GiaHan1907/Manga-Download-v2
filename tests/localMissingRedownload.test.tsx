/// Local detail page: a chapter whose file went missing (local_path set,
/// artifactOk=false — e.g. deleted in Explorer) is still listed under
/// "Đã có file" with "Thiếu file — cần tải lại", but it MUST also reappear
/// in the download queue list — the badge promises a re-download.
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => undefined) }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onResized: vi.fn().mockResolvedValue(() => undefined),
    onScaleChanged: vi.fn().mockResolvedValue(() => undefined),
    close: vi.fn().mockResolvedValue(undefined),
    hide: vi.fn().mockResolvedValue(undefined),
    show: vi.fn().mockResolvedValue(undefined),
    isMaximized: vi.fn().mockResolvedValue(false),
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    setFocus: vi.fn().mockResolvedValue(undefined),
  }),
}));

import { invoke } from "@tauri-apps/api/core";
import LocalMangaPage from "../src/features/library/LocalMangaPage";

const HEALTHY = {
  id: "c1",
  title: "Chương 1",
  number: 1,
  pageCount: 10,
  storageFormat: "cbz",
  sizeBytes: 1000,
  artifactOk: true,
  updatedAt: "2026-10-01T00:00:00.000Z",
};
const MISSING = {
  id: "c2",
  title: "Chương 2",
  number: 2,
  pageCount: 12,
  storageFormat: "cbz",
  sizeBytes: 2000,
  artifactOk: false,
  updatedAt: "2026-10-01T00:00:00.000Z",
};

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "app_get_local_manga":
        return Promise.resolve({
          id: "m1",
          sourceId: "truyenqq",
          sourceMangaId: "story-1",
          title: "Truyện Test",
          author: null,
          description: null,
          status: "ongoing",
          coverKey: null,
          chapterCount: 3,
          favorite: false,
          updatedAt: "2026-10-01T00:00:00.000Z",
          chapters: [
            { id: "c1", mangaId: "m1", sourceChapterId: "ch-1", title: "Chương 1", number: 1, pageCount: 10, downloaded: true, outputFormat: "cbz" },
            { id: "c2", mangaId: "m1", sourceChapterId: "ch-2", title: "Chương 2", number: 2, pageCount: 12, downloaded: true, outputFormat: "cbz" },
            { id: "c3", mangaId: "m1", sourceChapterId: "ch-3", title: "Chương 3", number: 3, pageCount: null, downloaded: false, outputFormat: null },
          ],
        });
      case "library_get_downloaded_chapters":
        return Promise.resolve({ chapters: [HEALTHY, MISSING] });
      case "app_get_reading_progress":
        return Promise.resolve(null);
      case "list_supported_sources":
        return Promise.resolve([
          {
            id: "truyenqq",
            displayName: "TruyenQQ",
            capabilities: ["search", "resolveUrl", "details", "pages"],
            status: "ready",
            attribution: null,
            enabled: true,
            builtin: true,
            baseUrl: null,
          },
        ]);
      case "library_get_watch":
        return Promise.resolve({ enabled: null, watchedAt: null, failCount: 0 });
      case "library_list_alerts":
        return Promise.resolve([]);
      case "app_list_read_history":
        return Promise.resolve({ items: [], total: 0, limit: 20, offset: 0 });
      default:
        return Promise.reject(new Error(`unexpected command ${cmd}`));
    }
  });
});

describe("missing artifacts return to the download list", () => {
  it("a chapter with artifactOk=false is offered for re-download", async () => {
    render(
      <MemoryRouter initialEntries={["/library/m1"]}>
        <Routes>
          <Route path="/library/:mangaId" element={<LocalMangaPage />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByText("Thiếu file — cần tải lại")).toBeDefined();
    // c2 (missing file) + c3 (never downloaded) — c1 stays out.
    expect(await screen.findByText(/Tải tất cả \(2\)/)).toBeDefined();
  });

  it("a missing-file row can be removed from the list", async () => {
    render(
      <MemoryRouter initialEntries={["/library/m1"]}>
        <Routes>
          <Route path="/library/:mangaId" element={<LocalMangaPage />} />
        </Routes>
      </MemoryRouter>,
    );
    const remove = await screen.findByRole("button", {
      name: "Gỡ chương Chương 2 khỏi danh sách",
    });
    fireEvent.click(remove);
    fireEvent.click(await screen.findByRole("button", { name: /Xác nhận gỡ khỏi danh sách/ }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("library_remove_chapter", { chapterId: "c2" }),
    );
  });
});
