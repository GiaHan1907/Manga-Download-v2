/// UI improvement plan (docs/plans/UI_IMPROVEMENT_PLAN.md) regression tests:
/// pure presentation math + the live-queue extensions (speed ring buffer,
/// action errors placement/self-clear).
import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor, render, screen } from "@testing-library/react";
import ToastViewport from "../src/components/ui/Toast";
import { pushToast } from "../src/components/ui/toastStore";

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
import { formatEtaSeconds, sparklinePoints } from "../src/types/downloads";

function task(overrides: Partial<DownloadTaskDto>): DownloadTaskDto {
  return {
    id: "t1",
    chapterId: "c1",
    status: "downloading",
    targetFormat: "cbz",
    priority: 0,
    completedPages: 1,
    totalPages: 4,
    attempts: 0,
    revision: 1,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  } as unknown as DownloadTaskDto;
}

describe("formatEtaSeconds", () => {
  it("hides meaningless or sub-minute estimates", () => {
    expect(formatEtaSeconds(null)).toBeNull();
    expect(formatEtaSeconds(undefined)).toBeNull();
    expect(formatEtaSeconds(Number.POSITIVE_INFINITY)).toBeNull();
    expect(formatEtaSeconds(-5)).toBeNull();
    expect(formatEtaSeconds(30)).toBeNull(); // < 1 min — flicker guard
  });
  it("renders minutes and hours", () => {
    expect(formatEtaSeconds(60)).toBe("còn ~1p");
    expect(formatEtaSeconds(210)).toBe("còn ~4p");
    expect(formatEtaSeconds(3720)).toBe("còn ~1h2p");
  });
});

describe("sparklinePoints", () => {
  it("needs two samples to have a shape", () => {
    expect(sparklinePoints([])).toBeNull();
    expect(sparklinePoints([1000])).toBeNull();
  });
  it("normalizes into the 100x28 box, max at the top", () => {
    const points = sparklinePoints([0, 500, 1000]);
    expect(points).not.toBeNull();
    const pairs = (points as string).split(" ");
    expect(pairs).toHaveLength(3);
    const [x0, y0] = (pairs[0] as string).split(",").map(Number);
    const [x2, y2] = (pairs[2] as string).split(",").map(Number);
    expect(x0).toBeCloseTo(0);
    expect(x2).toBeCloseTo(100);
    expect(y0).toBeGreaterThan(y2); // slowest first, highest sample lowest y
    expect(y2).toBeCloseTo(2); // max maps to top padding line
  });
});

describe("useDownloadQueue live telemetry", () => {
  beforeEach(() => {
    vi.mocked(downloadsApi.listTasks).mockReset();
    vi.mocked(downloadsApi.subscribe).mockReset();
    vi.mocked(downloadsApi.listTasks).mockResolvedValue({ items: [], total: 0 });
  });

  it("keeps a bounded speed history and clears it when the task goes terminal", async () => {
    let emit: ((event: unknown) => void) | undefined;
    vi.mocked(downloadsApi.subscribe).mockImplementation(async (handler) => {
      emit = handler as (event: unknown) => void;
      return () => undefined;
    });

    const { result } = renderHook(() => useDownloadQueue());
    await waitFor(() => expect(result.current.loading).toBe(false));

    for (let i = 0; i < 40; i += 1) {
      act(() =>
        emit?.({
          task: task({ revision: i + 2, completedPages: Math.min(i, 3) }),
          downloadedBytes: 1000 * (i + 1),
          speedBytesPerSecond: 100 + i,
        }),
      );
    }
    expect(result.current.speedHistory["t1"]).toHaveLength(30);
    expect(result.current.speedHistory["t1"]?.at(-1)).toBe(139);

    act(() =>
      emit?.({
        task: task({ revision: 100, status: "completed", completedPages: 4, totalPages: 4 }),
        downloadedBytes: 5000,
        speedBytesPerSecond: null,
      }),
    );
    expect(result.current.speedHistory["t1"]).toBeUndefined();
  });

  it("action rejection lands per-card and self-clears on the next revision", async () => {
    let emit: ((event: unknown) => void) | undefined;
    vi.mocked(downloadsApi.subscribe).mockImplementation(async (handler) => {
      emit = handler as (event: unknown) => void;
      return () => undefined;
    });
    vi.mocked(downloadsApi.pause).mockRejectedValue(new Error("pause bị từ chối"));

    const { result } = renderHook(() => useDownloadQueue());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await expect(result.current.actions.pause("t1")).rejects.toThrow();
    });
    expect(result.current.actionErrors["t1"]).toBe("pause bị từ chối");
    // Global banner stays reserved for snapshot-level failures.
    expect(result.current.error).toBeNull();

    act(() =>
      emit?.({
        task: task({ revision: 9 }),
        downloadedBytes: 4000,
        speedBytesPerSecond: 200,
      }),
    );
    expect(result.current.actionErrors["t1"]).toBeUndefined();
  });
});

describe("ToastViewport (C5)", () => {
  it("renders pushed messages, caps the stack at 3, and self-dismisses", () => {
    vi.useFakeTimers();
    try {
      render(<ToastViewport />);
      act(() => {
        pushToast("Đã tạm dừng tác vụ 1.");
        pushToast("Đã tạm dừng tác vụ 2.");
        pushToast("Đã tạm dừng tác vụ 3.");
        pushToast("Đã tạm dừng tác vụ 4.");
      });
      // Oldest dropped by the cap: only 2..4 visible.
      expect(screen.queryByText("Đã tạm dừng tác vụ 1.")).toBeNull();
      expect(screen.getByText("Đã tạm dừng tác vụ 4.")).toBeTruthy();
      act(() => {
        vi.advanceTimersByTime(4100);
      });
      expect(screen.queryByText("Đã tạm dừng tác vụ 4.")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
