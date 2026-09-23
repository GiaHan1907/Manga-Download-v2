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
