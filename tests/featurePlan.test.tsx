/// Feature plan v1.0.x (F2 + F5) frontend-boundary regressions: the three
/// new commands must be invoked with exactly the documented names/args and
/// nothing else may leak through the wrapper layer.
import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onResized: vi.fn().mockResolvedValue(() => undefined),
    onScaleChanged: vi.fn().mockResolvedValue(() => undefined),
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  clearDownloadHistory,
  retryAllFailedDownloads,
  cancelDownloadsForManga,
} from "../src/shared/api/desktop";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("F2/F5 IPC wrappers", () => {
  it("clear_download_history forwards statuses + olderThanDays only", async () => {
    vi.mocked(invoke).mockResolvedValue({ deleted: 2, skippedArtifact: 1, skippedOther: 0 });
    const outcome = await clearDownloadHistory({
      statuses: ["failed"],
      olderThanDays: 30,
    });
    expect(invoke).toHaveBeenCalledWith("clear_download_history", {
      statuses: ["failed"],
      olderThanDays: 30,
    });
    expect(outcome).toEqual({ deleted: 2, skippedArtifact: 1, skippedOther: 0 });
  });

  it("retry_all_failed_downloads takes no arguments", async () => {
    vi.mocked(invoke).mockResolvedValue(3);
    await expect(retryAllFailedDownloads()).resolves.toBe(3);
    expect(invoke).toHaveBeenCalledWith("retry_all_failed_downloads");
  });

  it("cancel_downloads_for_manga sends the mangaId (id only — never paths)", async () => {
    vi.mocked(invoke).mockResolvedValue(1);
    await expect(cancelDownloadsForManga("m1")).resolves.toBe(1);
    expect(invoke).toHaveBeenCalledWith("cancel_downloads_for_manga", { mangaId: "m1" });
  });
});

import { libraryImportCbz } from "../src/shared/api/desktop";

describe("F3b CBZ import IPC wrapper", () => {
  it("sends only the mangaId (+optional metadata) and never a path", async () => {
    vi.mocked(invoke).mockResolvedValue({
      mangaId: "m1",
      chapterId: "c1",
      number: 1,
      title: "Chương 1",
      pageCount: 3,
      sizeBytes: 1234,
    });
    const result = await libraryImportCbz({ mangaId: "m1" });
    expect(invoke).toHaveBeenCalledWith("library_import_cbz", { mangaId: "m1" });
    expect(result?.pageCount).toBe(3);
  });

  it("resolves null when the user cancels the native dialog", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    await expect(libraryImportCbz({ mangaId: "m1" })).resolves.toBeNull();
  });
});

import { queueChapterDownloadByUrl } from "../src/shared/api/desktop";

describe("paste-a-link IPC wrapper", () => {
  it("sends the user-pasted url and returns the standard queue result", async () => {
    const result = { tasks: [], reused: [], errors: [], queuedAt: "2026-09-23T00:00:00.000Z" };
    vi.mocked(invoke).mockResolvedValue(result);
    await expect(
      queueChapterDownloadByUrl({ url: "https://site.test/manga/m/ch-1/" }),
    ).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledWith("queue_chapter_download_by_url", {
      url: "https://site.test/manga/m/ch-1/",
    });
  });
});

import { revealDownload } from "../src/shared/api/desktop";

describe("reveal-download IPC wrapper", () => {
  it("passes only the task id when revealing one artifact", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await revealDownload({ taskId: "t1" });
    expect(invoke).toHaveBeenCalledWith("reveal_download", { taskId: "t1" });
  });
  it("opens the library root when no id is given", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await revealDownload();
    expect(invoke).toHaveBeenCalledWith("reveal_download", {});
  });
});

import { downloadsApi } from "../src/services/downloads.api";

describe("queueMany chunking (Rust caps 100 ids per call)", () => {
  it("splits 250 ids into 100/100/50 and aggregates one result", async () => {
    const ids = Array.from({ length: 250 }, (_, index) => `c${index}`);
    const calls: string[][] = [];
    vi.mocked(invoke).mockImplementation(async (_cmd: string, args?: Record<string, unknown>) => {
      const chunk = (args as { chapterIds: string[] }).chapterIds;
      calls.push(chunk);
      return {
        tasks: chunk.map((id) => ({ id, chapterId: id })),
        reused: [],
        errors: [],
        queuedAt: "2026-09-23T00:00:00.000Z",
      };
    });
    const out = await downloadsApi.queueMany({ chapterIds: ids });
    expect(calls.map((chunk) => chunk.length)).toEqual([100, 100, 50]);
    expect(calls.flat()).toEqual(ids);
    expect(out.tasks).toHaveLength(250);
  });

  it("empty selection performs no IPC at all", async () => {
    vi.mocked(invoke).mockReset();
    const out = await downloadsApi.queueMany({ chapterIds: [] });
    expect(invoke).not.toHaveBeenCalled();
    expect(out.tasks).toEqual([]);
  });
});

import {
  libraryGetWatch,
  librarySetWatch,
  libraryListAlerts,
  libraryAckAlerts,
  watchRunNow,
} from "../src/shared/api/desktop";

describe("F1 watch IPC wrappers", () => {
  it("sends ids only and mirrors the documented command names", async () => {
    vi.mocked(invoke).mockResolvedValue({ enabled: null, watchedAt: null, failCount: 0 });
    await libraryGetWatch("m1");
    expect(invoke).toHaveBeenCalledWith("library_get_watch", { mangaId: "m1" });

    await librarySetWatch({ mangaId: "m1", enabled: false });
    expect(invoke).toHaveBeenCalledWith("library_set_watch", { mangaId: "m1", enabled: false });

    // Security-review follow-up: NULL restores "follow the global setting".
    await librarySetWatch({ mangaId: "m1", enabled: null });
    expect(invoke).toHaveBeenCalledWith("library_set_watch", { mangaId: "m1", enabled: null });

    vi.mocked(invoke).mockResolvedValue([]);
    await libraryListAlerts("m1");
    expect(invoke).toHaveBeenCalledWith("library_list_alerts", { mangaId: "m1" });

    await libraryAckAlerts("m1");
    expect(invoke).toHaveBeenCalledWith("library_ack_alerts", { mangaId: "m1" });

    vi.mocked(invoke).mockResolvedValue({ checked: 2, alertedChapters: 1, failed: 0 });
    const report = await watchRunNow();
    expect(invoke).toHaveBeenCalledWith("watch_run_now");
    expect(report.alertedChapters).toBe(1);
  });
});

import { listReadHistory, clearReadHistory } from "../src/shared/api/desktop";

describe("F4 read-history IPC wrappers", () => {
  it("lists with paging filters and clears per manga (ids only)", async () => {
    vi.mocked(invoke).mockResolvedValue({ items: [], total: 0, limit: 20, offset: 0 });
    await listReadHistory({ mangaId: "m1", limit: 20, offset: 0 });
    expect(invoke).toHaveBeenCalledWith("app_list_read_history", {
      mangaId: "m1",
      limit: 20,
      offset: 0,
    });

    await listReadHistory({ limit: 10, offset: 0 });
    expect(invoke).toHaveBeenLastCalledWith("app_list_read_history", { limit: 10, offset: 0 });

    vi.mocked(invoke).mockResolvedValue(4);
    await expect(clearReadHistory("m1")).resolves.toBe(4);
    expect(invoke).toHaveBeenCalledWith("app_clear_read_history", { mangaId: "m1" });
  });
});
