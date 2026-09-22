/// Whole-project review round 3 regression tests: the download action table
/// must mirror the Rust lifecycle (pause rejected while verifying/exporting),
/// and the queue mirror must stay monotonic across out-of-order snapshots
/// and event frames. Also pins the reader blob-cache disposal guard.
import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

import type { DownloadTaskDto } from "../src/types/backend";

vi.mock("../src/services/downloads.api", () => ({
  downloadsApi: {
    listTasks: vi.fn(),
    subscribe: vi.fn(),
    pause: vi.fn(),
    resume: vi.fn(),
    cancel: vi.fn(),
    retry: vi.fn(),
    setPriority: vi.fn(),
  },
}));

import { downloadsApi } from "../src/services/downloads.api";
import { useDownloadQueue } from "../src/features/downloads/useDownloadQueue";
import { allowedDownloadActions } from "../src/types/downloads";

function task(id: string, revision: number, status = "downloading"): DownloadTaskDto {
  return {
    id,
    chapterId: `c-${id}`,
    status,
    targetFormat: "cbz",
    priority: 0,
    completedPages: 1,
    totalPages: 2,
    attempts: 0,
    revision,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  } as unknown as DownloadTaskDto;
}

describe("download action table mirrors the Rust lifecycle", () => {
  it("does not offer pause during the verify/export commit window", () => {
    expect(allowedDownloadActions("verifying")).toEqual(["cancel"]);
    expect(allowedDownloadActions("exporting")).toEqual(["cancel"]);
    expect(allowedDownloadActions("downloading")).toContain("pause");
    expect(allowedDownloadActions("resolving")).toContain("pause");
    expect(allowedDownloadActions("queued")).toEqual(["pause", "cancel"]);
    expect(allowedDownloadActions("paused")).toEqual(["resume", "cancel"]);
  });
});

describe("useDownloadQueue monotonic merge", () => {
  beforeEach(() => {
    vi.mocked(downloadsApi.listTasks).mockReset();
    vi.mocked(downloadsApi.subscribe).mockReset();
    vi.mocked(downloadsApi.subscribe).mockResolvedValue(() => undefined);
  });

  it("an older snapshot resolving last cannot roll back newer state", async () => {
    const gates: ((value: unknown) => void)[] = [];
    vi.mocked(downloadsApi.listTasks).mockImplementation(
      () => new Promise((resolve) => gates.push(resolve)),
    );

    const { result } = renderHook(() => useDownloadQueue());
    await waitFor(() => expect(gates.length).toBe(1));

    // Second refresh (page change) resolves FIRST with a newer revision.
    act(() => result.current.goPage(100));
    await waitFor(() => expect(gates.length).toBe(2));
    act(() =>
      gates[1]({
        items: [task("t1", 7, "completed")],
        total: 1,
      }),
    );
    await waitFor(() => expect(result.current.tasks[0]?.revision).toBe(7));

    // The stale first response lands late: it must not downgrade the card
    // nor lower the revision guard for subsequent events.
    act(() =>
      gates[0]({
        items: [task("t1", 5)],
        total: 1,
      }),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(result.current.tasks[0]?.revision).toBe(7);
    expect(result.current.tasks[0]?.status).toBe("completed");
  });

  it("event frames below the last applied revision are dropped", async () => {
    let emit: ((event: unknown) => void) | undefined;
    vi.mocked(downloadsApi.subscribe).mockImplementation(async (handler) => {
      emit = handler as (event: unknown) => void;
      return () => undefined;
    });
    vi.mocked(downloadsApi.listTasks).mockResolvedValue({
      items: [task("t1", 3)],
      total: 1,
    });

    const { result } = renderHook(() => useDownloadQueue());
    await waitFor(() => expect(result.current.tasks.length).toBe(1));

    act(() => emit?.({ task: task("t1", 4), downloadedBytes: 10, speedBytesPerSecond: null }));
    expect(result.current.tasks[0]?.revision).toBe(4);

    act(() => emit?.({ task: task("t1", 2), downloadedBytes: 1, speedBytesPerSecond: null }));
    expect(result.current.tasks[0]?.revision).toBe(4);
  });
});

describe("usePageImages blob lifecycle", () => {
  it("revokes object URLs created by fetches that resolve after unmount", async () => {
    const created: string[] = [];
    const revoked: string[] = [];
    (URL as unknown as { createObjectURL: (b: Blob) => string }).createObjectURL = vi.fn(
      () => {
        const url = `blob:test-${created.length}`;
        created.push(url);
        return url;
      },
    );
    (URL as unknown as { revokeObjectURL: (u: string) => void }).revokeObjectURL = vi.fn(
      (url: string) => {
        revoked.push(url);
      },
    );

    let resolveFetch: ((value: { bytes: Uint8Array; mime: string }) => void) | undefined;
    const fetchPage = vi.fn(
      () =>
        new Promise<{ bytes: Uint8Array; mime: string }>((resolve) => {
          resolveFetch = resolve;
        }),
    );

    const { usePageImages } = await import("../src/features/read/usePageImages");
    const { unmount } = renderHook(() => usePageImages("ch1", 0, "single", 5, fetchPage));
    await waitFor(() => expect(resolveFetch).toBeDefined());

    unmount();
    act(() => resolveFetch?.({ bytes: new Uint8Array([1, 2, 3]), mime: "image/png" }));
    await new Promise((resolve) => setTimeout(resolve, 0));

    // The late blob URL (if created at all) must be revoked, never kept.
    const leaked = created.filter((url) => !revoked.includes(url));
    expect(leaked).toEqual([]);
  });
});
