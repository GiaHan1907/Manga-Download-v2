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
