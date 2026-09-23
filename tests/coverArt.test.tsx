/// Cover-art boundary regressions: the get_manga_cover wrapper must send only
/// opaque ids/keys and normalize every binary transport shape to Uint8Array;
/// sniffImageMime labels the blob from magic bytes (Rust already validated).
import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { getMangaCover } from "../src/shared/api/desktop";
import { sniffImageMime } from "../src/shared/lib/imageMime";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("getMangaCover IPC wrapper", () => {
  it("forwards sourceId/sourceMangaId/coverKey and returns bytes", async () => {
    vi.mocked(invoke).mockResolvedValue(new Uint8Array([0xff, 0xd8, 0xff, 0x01]).buffer);
    const bytes = await getMangaCover({
      sourceId: "madara-a-com",
      sourceMangaId: "ninja",
      coverKey: "/wp/a.jpg",
    });
    expect(invoke).toHaveBeenCalledWith("get_manga_cover", {
      sourceId: "madara-a-com",
      sourceMangaId: "ninja",
      coverKey: "/wp/a.jpg",
    });
    expect(Array.from(bytes)).toEqual([0xff, 0xd8, 0xff, 1]);
  });

  it("normalizes number[] payloads to Uint8Array", async () => {
    vi.mocked(invoke).mockResolvedValue([0x89, 0x50, 0x4e, 0x47]);
    const bytes = await getMangaCover({ sourceId: "s", sourceMangaId: "m", coverKey: "/k" });
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(Array.from(bytes)).toEqual([0x89, 0x50, 0x4e, 0x47]);
  });

  it("passes through Uint8Array payloads unchanged", async () => {
    const raw = new Uint8Array([1, 2, 3]);
    vi.mocked(invoke).mockResolvedValue(raw);
    const bytes = await getMangaCover({ sourceId: "s", sourceMangaId: "m", coverKey: "/k" });
    expect(bytes).toBe(raw);
  });
});

describe("sniffImageMime", () => {
  it("maps magic bytes to image MIME types", () => {
    expect(sniffImageMime(new Uint8Array([0xff, 0xd8, 0xff]))).toBe("image/jpeg");
    expect(
      sniffImageMime(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x1a, 0x0a, 0x00])),
    ).toBe("image/png");
    expect(sniffImageMime(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))).toBe("image/gif");
    const webp = new Uint8Array(12);
    webp.set([0x52, 0x49, 0x46, 0x46], 0);
    webp.set([0x57, 0x45, 0x42, 0x50], 8);
    expect(sniffImageMime(webp)).toBe("image/webp");
  });

  it("falls back for unknown or short payloads", () => {
    expect(sniffImageMime(new Uint8Array([0x00, 0x01]))).toBe("application/octet-stream");
    expect(sniffImageMime(new Uint8Array([]))).toBe("application/octet-stream");
  });
});
