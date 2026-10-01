import { describe, it, expect, vi, beforeEach } from "vitest";
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

let emitEvent: ((payload: unknown) => void) | null = null;
const unlisten = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: (msg: { payload: unknown }) => void) => {
    emitEvent = (payload: unknown) => handler({ payload });
    return Promise.resolve(unlisten);
  }),
}));

import { invoke } from "@tauri-apps/api/core";

function baseTask(id: string, status: string, revision: number) {
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
    revision,
    finishedAt: null,
    outputRelpath: null,
    chapterTitle: `Chương ${id}`,
    mangaTitle: `Truyện ${id}`,
    createdAt: "2026-09-22T00:00:00.000Z",
    updatedAt: "2026-09-22T00:00:00.000Z",
  };
}

function mockQueue(tasks: ReturnType<typeof baseTask>[]) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "app_list_local_download_tasks") {
      return Promise.resolve({ items: tasks, total: tasks.length, limit: 100, offset: 0 });
    }
    if (cmd === "pause_download" && args) {
      const paused = tasks.map((t) => (t.id === args.taskId ? { ...t, status: "paused", revision: t.revision + 1 } : t));
      return Promise.resolve(paused.find((t) => t.id === args.taskId));
    }
    if (cmd === "remove_download_task") return Promise.resolve(true);
    if (cmd === "app_get_info") return Promise.resolve({ name: "x", version: "0.1.0", platform: "windows" });
    if (cmd === "app_health_check") return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 4 });
    return Promise.reject(new Error(`unexpected command ${cmd}`));
  });
}

function renderDownloads() {
  return render(
    <MemoryRouter initialEntries={["/downloads"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

describe("T06 Downloads manager (real backend)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    emitEvent = null;
    window.location.hash = "#/downloads";
  });

  it("renders real tasks with status and per-status actions", async () => {
    mockQueue([baseTask("t1", "downloading", 3), baseTask("t2", "completed", 5)]);
    renderDownloads();
    expect(await screen.findByText(/Hàng đợi tải — 2\/2 tác vụ/)).toBeDefined();
    expect(screen.getByText("Truyện t1")).toBeDefined();
    const activeCard = screen.getByText("Truyện t1").closest("article") as HTMLElement;
    const doneCard = screen.getByText("Truyện t2").closest("article") as HTMLElement;
    expect(within(activeCard).getByText("Đang tải")).toBeDefined();
    expect(within(activeCard).getByRole("button", { name: /Tạm dừng/ })).toBeDefined();
    expect(within(activeCard).getByRole("button", { name: /Hủy/ })).toBeDefined();
    expect(within(doneCard).queryByRole("button", { name: /Tạm dừng/ })).toBeNull();
    expect(within(doneCard).queryByText(/Đã xác minh/)).toBeNull(); // outputRelpath null in fixture
  });

  it("a failed task offers retry and removal, never a cancel the backend rejects", async () => {
    mockQueue([baseTask("t9", "failed", 1)]);
    renderDownloads();
    const card = (await screen.findByText("Truyện t9")).closest("article") as HTMLElement;
    expect(within(card).queryByRole("button", { name: /Hủy/ })).toBeNull();
    expect(within(card).getByRole("button", { name: /Thử lại/ })).toBeDefined();
    fireEvent.click(within(card).getByRole("button", { name: /Gỡ khỏi danh sách/ }));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("remove_download_task", { taskId: "t9" });
    });
  });

  it("invokes pause for the right task and re-renders the authoritative snapshot", async () => {    mockQueue([baseTask("t1", "downloading", 3)]);
    renderDownloads();
    const card = (await screen.findByText("Truyện t1")).closest("article") as HTMLElement;
    fireEvent.click(within(card).getByRole("button", { name: /Tạm dừng/ }));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("pause_download", { taskId: "t1" });
    });
  });

  it("global gate: reads paused state, shows marker, resumes on click", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_download_queue_paused") return Promise.resolve(true);
      if (cmd === "set_download_queue_paused") return Promise.resolve(1);
      if (cmd === "app_list_local_download_tasks") {
        return Promise.resolve({ items: [], total: 0, limit: 100, offset: 0 });
      }
      if (cmd === "app_get_info") return Promise.resolve({ name: "x", version: "0.1.0", platform: "windows" });
      if (cmd === "app_health_check") return Promise.resolve({ status: "ok", database: "ready", schemaVersion: 4 });
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });
    renderDownloads();
    const resumeButton = await screen.findByRole("button", { name: "Tiếp tục toàn bộ" });
    expect(screen.getByText("Hàng đợi đang tạm dừng")).toBeDefined();
    fireEvent.click(resumeButton);
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_download_queue_paused", { paused: false });
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Dừng toàn bộ" })).toBeDefined();
    });
  });

  it("drops an out-of-order event with an older revision (T06-22 race)", async () => {
    mockQueue([baseTask("t1", "downloading", 5)]);
    renderDownloads();
    await screen.findByText("Truyện t1");
    expect(emitEvent).not.toBeNull();
    // Older revision frame must not roll back the card.
    emitEvent?.({ task: baseTask("t1", "queued", 4), downloadedBytes: 0, speedBytesPerSecond: null, occurredAt: "x" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    const card = screen.getByText("Truyện t1").closest("article") as HTMLElement;
    expect(within(card).getByText("Đang tải")).toBeDefined();
    // Newer revision applies.
    emitEvent?.({ task: baseTask("t1", "exporting", 6), downloadedBytes: 100, speedBytesPerSecond: null, occurredAt: "y" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    const card2 = screen.getByText("Truyện t1").closest("article") as HTMLElement;
    expect(within(card2).getByText("Đang đóng gói")).toBeDefined();
  });

  it("unsubscribes on teardown (listener cleanup)", async () => {
    mockQueue([baseTask("t1", "downloading", 1)]);
    const view = renderDownloads();
    await screen.findByText("Truyện t1");
    view.unmount();
    expect(unlisten).toHaveBeenCalled();
  });
});
