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
  mangaId: string | null;
  chapterNumber: number | null;
  createdAt: string;
  updatedAt: string;
};

function task(id: string, number: number | null, status = "queued", overrides: Partial<Task> = {}): Task {
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
    mangaId: "manga-ninja",
    chapterNumber: number,
    createdAt: `2026-10-07T00:00:0${number ?? 0}.000Z`,
    updatedAt: "2026-10-07T00:00:00.000Z",
    ...overrides,
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

/** Card order as `truyện|chương`, so rows of two mangas stay distinguishable. */
function renderedRows(): string[] {
  return Array.from(document.querySelectorAll(".download-card")).map((card) => {
    const manga = card.querySelector(".download-card__title")?.textContent ?? "";
    const meta = card.querySelector(".download-card__meta")?.textContent ?? "";
    return `${manga}|${meta.replace(/\s*·.*$/, "").trim()}`;
  });
}

/** The order the cards are actually rendered in, across every manga group. */
function renderedChapters(): string[] {
  return Array.from(document.querySelectorAll(".download-card")).map((card) => {
    const meta = card.querySelector(".download-card__meta")?.textContent ?? "";
    return meta.replace(/\s*·.*$/, "").trim();
  });
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

  it("gom chương theo truyện: truyện xếp hàng trước lên trên, chương trong truyện tăng dần", async () => {
    // Ninja entered the queue at :01, Vua at :07 — even though Vua's newest
    // chapter was queued later and arrives first in this (shuffled) payload,
    // the group ranks must follow the earliest chapter of each manga.
    const vua = { mangaId: "manga-vua", mangaTitle: "Vua Pháp Thuật" };
    mockQueue([
      task("v2", 2, "queued", { ...vua, createdAt: "2026-10-07T00:00:08.000Z" }),
      task("n2", 2, "queued", { createdAt: "2026-10-07T00:00:04.000Z" }),
      task("v1", 1, "queued", { ...vua, createdAt: "2026-10-07T00:00:07.000Z" }),
      task("n1", 1, "queued", { createdAt: "2026-10-07T00:00:01.000Z" }),
    ]);
    await renderPage();
    await waitFor(() =>
      expect(renderedRows()).toEqual([
        "Ninja Loạn Thị|Chương 1",
        "Ninja Loạn Thị|Chương 2",
        "Vua Pháp Thuật|Chương 1",
        "Vua Pháp Thuật|Chương 2",
      ]),
    );
    // The grouping is visible, not just an ordering coincidence.
    const heads = Array.from(document.querySelectorAll(".download-group__title")).map((node) =>
      (node.textContent ?? "").replace(/\s+/g, " ").trim(),
    );
    expect(heads).toEqual(["Ninja Loạn Thị · 2 chương", "Vua Pháp Thuật · 2 chương"]);
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
