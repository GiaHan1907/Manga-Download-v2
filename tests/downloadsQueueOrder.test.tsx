import { describe, it, expect, vi, beforeEach } from "vitest";
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

let emitEvent: ((payload: unknown) => void) | null = null;
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: (msg: { payload: unknown }) => void) => {
    emitEvent = (payload: unknown) => handler({ payload });
    return Promise.resolve(vi.fn());
  }),
}));

import { invoke } from "@tauri-apps/api/core";

type Task = {
  id: string;
  chapterId: string;
  status: string;
  targetFormat: string;
  priority: number;
  completedPages: number;
  totalPages: number;
  attempts: number;
  nextRetryAt: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  revision: number;
  finishedAt: string | null;
  outputRelpath: string | null;
  chapterTitle: string;
  mangaTitle: string;
  chapterNumber: number | null;
  createdAt: string;
  updatedAt: string;
};

function task(id: string, number: number | null, status = "queued"): Task {
  return {
    id,
    chapterId: `ch-${id}`,
    status,
    targetFormat: "cbz",
    priority: 0,
    completedPages: 1,
    totalPages: 4,
    attempts: 0,
    nextRetryAt: null,
    errorCode: null,
    errorMessage: null,
    revision: 1,
    finishedAt: null,
    outputRelpath: null,
    chapterTitle: `Chương ${number ?? "?"}`,
    mangaTitle: "Ninja Loạn Thị",
    chapterNumber: number,
    createdAt: `2026-10-07T00:00:0${number ?? 0}.000Z`,
    updatedAt: "2026-10-07T00:00:00.000Z",
  };
}

function mockQueue(tasks: Task[]) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "app_list_local_download_tasks") {
      return Promise.resolve({ items: tasks, total: tasks.length, limit: 100, offset: 0 });
    }
    if (cmd === "get_download_queue_paused") return Promise.resolve(0);
    return Promise.resolve(null);
  });
}

/** The order the cards are actually rendered in, read off the grid. */
function renderedChapters(): string[] {
  const grid = document.querySelector(".downloads-grid");
  if (!grid) return [];
  return Array.from(grid.querySelectorAll("p, div"))
    .map((node) => node.textContent ?? "")
    .filter((text) => /^Chương [\d?]+ ·/.test(text))
    .map((text) => text.replace(/\s*·.*$/, "").trim());
}

async function renderPage() {
  render(
    <MemoryRouter initialEntries={["/downloads"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
  await screen.findByText(/Hàng đợi tải/);
}

describe("Trang Tải xuống: thứ tự chương và mục đã xong", () => {
  beforeEach(() => {
    emitEvent = null;
    vi.mocked(invoke).mockReset();
  });

  it("đặt lại chương theo thứ tự tăng dần dù server trả lộn xộn", async () => {
    mockQueue([task("t3", 3), task("t1", 1), task("tx", null), task("t2", 2)]);
    await renderPage();
    await waitFor(() => expect(renderedChapters()).toEqual(["Chương 1", "Chương 2", "Chương 3", "Chương ?"]));
  });

  it("mặc định KHÔNG hiện chương đã hoàn thành, và chip Hoàn thành vẫn xem được", async () => {
    mockQueue([task("t1", 1, "completed"), task("t2", 2, "downloading")]);
    await renderPage();
    expect(renderedChapters()).toEqual(["Chương 2"]);
    // The row is not deleted — it is one chip away.
    fireEvent.click(screen.getByRole("button", { name: /Hoàn thành/ }));
    await waitFor(() => expect(renderedChapters()).toEqual(["Chương 1"]));
  });

  it("chương vừa xong biến mất khỏi hàng đợi ngay khi event về, không cần tải lại", async () => {
    const running = task("t1", 1, "downloading");
    mockQueue([running, task("t2", 2, "queued")]);
    await renderPage();
    expect(renderedChapters()).toEqual(["Chương 1", "Chương 2"]);

    emitEvent?.({
      task: { ...running, status: "completed", completedPages: 4, revision: 2 },
      downloadedBytes: 0,
      speedBytesPerSecond: 0,
    });
    await waitFor(() => expect(renderedChapters()).toEqual(["Chương 2"]));
    // Still reachable, because nothing was deleted.
    fireEvent.click(screen.getByRole("button", { name: /Hoàn thành/ }));
    await waitFor(() => expect(renderedChapters()).toEqual(["Chương 1"]));
  });
});
