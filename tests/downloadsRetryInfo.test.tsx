/// The queue used to hide the retry ladder entirely: a task waiting out its
/// backoff is `queued` again (the engine re-queues with attempts+1 and a due
/// time), so it looked like a fresh queue entry, and a terminal failure showed
/// only `[SOURCE_…] message` with no hint about what to do next.
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
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => undefined)),
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

function task(overrides: Partial<Task>): Task {
  return {
    id: "t1",
    chapterId: "c1",
    status: "queued",
    targetFormat: "cbz",
    priority: 0,
    completedPages: 4,
    totalPages: 20,
    attempts: 0,
    nextRetryAt: null,
    errorCode: null,
    errorMessage: null,
    revision: 3,
    finishedAt: null,
    outputRelpath: null,
    chapterTitle: "Chương 9",
    mangaTitle: "Vua Pháp Thuật",
    mangaId: "m-vua",
    chapterNumber: 9,
    createdAt: "2026-10-26T04:00:00.000Z",
    updatedAt: "2026-10-26T04:05:00.000Z",
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

function renderDownloads() {
  render(
    <MemoryRouter initialEntries={["/downloads"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("Hàng đợi: nói rõ lần thử lại và bước tiếp theo", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("a task waiting out its backoff says which attempt is coming and when", async () => {
    const due = new Date(Date.now() + 10 * 60_000).toISOString();
    mockQueue([task({ status: "queued", attempts: 2, nextRetryAt: due })]);
    renderDownloads();
    // Attempts already counts finished tries, so the next one is attempts+1.
    expect(await screen.findByText(/Tự thử lại lần 3 sau 10 phút/)).toBeDefined();
  });

  it("a finished task never shows a countdown", async () => {
    mockQueue([
      task({ status: "completed", attempts: 0, nextRetryAt: null, outputRelpath: "vua/ch9.cbz" }),
    ]);
    renderDownloads();
    // Completed rows sit behind their own chip — the queue filter hides them.
    fireEvent.click(await screen.findByRole("button", { name: /Hoàn thành/ }));
    expect(await screen.findByText(/Chương 9/)).toBeDefined();
    expect(screen.queryByText(/Tự thử lại/)).toBeNull();
  });

  it("a terminal failure leads with the message, then the next step and the code", async () => {
    mockQueue([
      task({
        status: "failed",
        attempts: 5,
        errorCode: "SOURCE_NOT_FOUND",
        errorMessage: "Không tìm thấy nội dung trên nguồn (có thể link đã hết hạn).",
      }),
    ]);
    renderDownloads();
    expect(
      await screen.findByText("Không tìm thấy nội dung trên nguồn (có thể link đã hết hạn)."),
    ).toBeDefined();
    expect(
      screen.getByText("Nguồn có thể đã bỏ chương này — mở chi tiết truyện để kiểm tra."),
    ).toBeDefined();
    expect(screen.getByText("Đã thử 5 lần · không tự thử lại nữa")).toBeDefined();
    // The wire code is reference material, not the headline.
    expect(screen.getByText("Mã lỗi: SOURCE_NOT_FOUND")).toBeDefined();
    expect(screen.queryByText(/^\[SOURCE_NOT_FOUND\]/)).toBeNull();
    await waitFor(() => expect(screen.getByRole("button", { name: /^Thử lại$/ })).toBeDefined());
  });
});