import { beforeEach, describe, expect, it, vi } from "vitest";

const mockWindow = {
  minimize: vi.fn(),
  toggleMaximize: vi.fn(),
  close: vi.fn(),
  isMaximized: vi.fn(),
};

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => mockWindow),
}));

import { windowService } from "../src/services/windowService";

describe("windowService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockWindow.minimize.mockResolvedValue(undefined);
    mockWindow.toggleMaximize.mockResolvedValue(undefined);
    mockWindow.close.mockResolvedValue(undefined);
    mockWindow.isMaximized.mockResolvedValue(false);
  });

  it("wraps minimize, maximize and close commands", async () => {
    await expect(windowService.minimize()).resolves.toBe(true);
    await expect(windowService.toggleMaximize()).resolves.toBe(true);
    await expect(windowService.close()).resolves.toBe(true);

    expect(mockWindow.minimize).toHaveBeenCalledTimes(1);
    expect(mockWindow.toggleMaximize).toHaveBeenCalledTimes(1);
    expect(mockWindow.close).toHaveBeenCalledTimes(1);
  });

  it("returns false when a window command rejects", async () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    mockWindow.minimize.mockRejectedValue(new Error("denied"));

    await expect(windowService.minimize()).resolves.toBe(false);
    expect(consoleSpy).toHaveBeenCalledOnce();

    consoleSpy.mockRestore();
  });
});
