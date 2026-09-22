import { beforeEach, describe, expect, it } from "vitest";
import { services, resetMockState } from "../src/services/provider";
import { mockSettingsService } from "../src/services/mock/mockServices";
import { ALL_CHAPTERS, MANGA_SUMMARIES, SEED_TASKS, getMangaSummary } from "../src/services/mock/fixtures";

describe("mock fixture integrity", () => {
  it("has at least 8 manga across 2 sources", () => {
    expect(MANGA_SUMMARIES.length).toBeGreaterThanOrEqual(8);
    expect(new Set(MANGA_SUMMARIES.map((m) => m.sourceId)).size).toBe(2);
  });

  it("keeps referential integrity between manga, chapters and tasks", () => {
    const mangaIds = new Set(MANGA_SUMMARIES.map((m) => m.id));
    for (const chapter of ALL_CHAPTERS) {
      expect(mangaIds.has(chapter.mangaId)).toBe(true);
    }
    for (const task of SEED_TASKS) {
      expect(mangaIds.has(task.mangaId)).toBe(true);
      const chapter = ALL_CHAPTERS.find((c) => c.id === task.chapterId);
      expect(chapter).toBeDefined();
      expect(chapter?.mangaId).toBe(task.mangaId);
    }
  });

  it("provides a reader-enabled chapter with at least 3 sample pages", async () => {
    const pages = await services.reader.getPages("manga-001-ch-01");
    expect(pages.length).toBeGreaterThanOrEqual(3);
    expect(pages.every((p) => p.src.length > 0)).toBe(true);
  });
});

describe("catalog service", () => {
  beforeEach(resetMockState);

  it("searches accent-insensitively by title", async () => {
    const results = await services.catalog.search("hanh trinh", []);
    expect(results.map((m) => m.id)).toContain("manga-001");
  });

  it("filters by source", async () => {
    const results = await services.catalog.search("", ["src-beta"]);
    expect(results.length).toBeGreaterThan(0);
    expect(results.every((m) => m.sourceId === "src-beta")).toBe(true);
  });

  it("returns empty for unmatched query", async () => {
    expect(await services.catalog.search("zzz-khong-ton-tai", [])).toEqual([]);
  });

  it("resolves the documented demo URL locally", async () => {
    const manga = await services.catalog.resolveDemoUrl("https://demo.manga1989.example/manga/manga-001");
    expect(manga?.id).toBe("manga-001");
  });

  it("rejects unsupported and malformed URLs without network", async () => {
    expect(await services.catalog.resolveDemoUrl("https://evil.example/manga/manga-001")).toBeNull();
    expect(await services.catalog.resolveDemoUrl("not a url")).toBeNull();
    expect(await services.catalog.resolveDemoUrl("https://demo.manga1989.example/manga/manga-999")).toBeNull();
  });

  it("returns null for unknown manga id", async () => {
    expect(await services.catalog.getManga("manga-999")).toBeNull();
  });
});

describe("download service", () => {
  beforeEach(resetMockState);

  it("queues new chapters and skips duplicates", async () => {
    const first = await services.downloads.queueChapters(["manga-002-ch-02", "manga-002-ch-03"]);
    expect(first).toEqual({ added: 2, skipped: 0 });

    const second = await services.downloads.queueChapters(["manga-002-ch-02"]);
    expect(second).toEqual({ added: 0, skipped: 1 });
  });

  it("skips unknown chapters", async () => {
    const result = await services.downloads.queueChapters(["chapter-does-not-exist"]);
    expect(result).toEqual({ added: 0, skipped: 1 });
  });

  it("generates deterministic task ids reproducible after reset", async () => {
    await services.downloads.queueChapters(["manga-004-ch-01"]);
    const idFirst = (await services.downloads.listTasks()).find((t) => t.chapterId === "manga-004-ch-01")?.id;

    resetMockState();
    await services.downloads.queueChapters(["manga-004-ch-01"]);
    const idSecond = (await services.downloads.listTasks()).find((t) => t.chapterId === "manga-004-ch-01")?.id;

    expect(idFirst).toBe("demo-task-001");
    expect(idSecond).toBe(idFirst);
  });

  it("applies valid transitions", async () => {
    await services.downloads.transition("task-003", "pause");
    let tasks = await services.downloads.listTasks();
    expect(tasks.find((t) => t.id === "task-003")?.status).toBe("paused");

    await services.downloads.transition("task-003", "resume");
    tasks = await services.downloads.listTasks();
    expect(tasks.find((t) => t.id === "task-003")?.status).toBe("queued");

    await services.downloads.transition("task-002", "retry");
    tasks = await services.downloads.listTasks();
    const retried = tasks.find((t) => t.id === "task-002");
    expect(retried?.status).toBe("queued");
    expect(retried?.completedPages).toBe(0);
  });

  it("rejects invalid transitions and missing tasks", async () => {
    await expect(services.downloads.transition("task-001", "pause")).rejects.toThrow(/Không thể/);
    await expect(services.downloads.transition("task-003", "retry")).rejects.toThrow(/Không thể/);
    await expect(services.downloads.transition("task-999", "pause")).rejects.toThrow();
  });
});

describe("library service", () => {
  beforeEach(resetMockState);

  it("toggles favorites and syncs lists", async () => {
    expect(await services.library.isFavorite("manga-003")).toBe(false);
    expect(await services.library.toggleFavorite("manga-003")).toBe(true);
    expect((await services.library.listFavorites()).map((m) => m.id)).toContain("manga-003");
    expect(await services.library.toggleFavorite("manga-003")).toBe(false);
    expect((await services.library.listFavorites()).map((m) => m.id)).not.toContain("manga-003");
  });

  it("removes from library without touching favorites", async () => {
    await services.library.removeFromLibrary("manga-001");
    expect((await services.library.listManga()).map((m) => m.id)).not.toContain("manga-001");
    expect((await services.library.listFavorites()).map((m) => m.id)).toContain("manga-001");
  });

  it("rejects favorite for unknown manga", async () => {
    await expect(services.library.toggleFavorite("manga-999")).rejects.toThrow();
  });
});

describe("reader service", () => {
  beforeEach(resetMockState);

  it("stores and returns reading progress", async () => {
    expect(await services.reader.getProgress("manga-001")).toBeNull();
    await services.reader.setProgress({ mangaId: "manga-001", chapterId: "manga-001-ch-01", page: 2 });
    expect(await services.reader.getProgress("manga-001")).toEqual({
      mangaId: "manga-001",
      chapterId: "manga-001-ch-01",
      page: 2,
    });
  });

  it("rejects mismatched progress data", async () => {
    await expect(
      services.reader.setProgress({ mangaId: "manga-002", chapterId: "manga-001-ch-01", page: 0 }),
    ).rejects.toThrow();
  });
});

describe("settings mock service (sandbox backend, kept for demo screens)", () => {
  beforeEach(resetMockState);

  it("validates ranges and resets", async () => {
    await expect(
      mockSettingsService.save({
        defaultReaderMode: "single",
        defaultReadingDirection: "ltr",
        chapterConcurrency: 99,
        imageConcurrency: 4,
        outputFormat: "cbz",
      }),
    ).rejects.toThrow();

    await mockSettingsService.save({
      defaultReaderMode: "double",
      defaultReadingDirection: "ltr",
      chapterConcurrency: 3,
      imageConcurrency: 6,
      outputFormat: "folder",
    });
    expect((await mockSettingsService.get()).chapterConcurrency).toBe(3);

    await mockSettingsService.reset();
    expect((await mockSettingsService.get()).chapterConcurrency).toBe(2);
  });
});

describe("source enablement", () => {
  beforeEach(resetMockState);

  it("disabling a source removes it from enabled list", async () => {
    await services.catalog.setSourceEnabled("src-beta", false);
    const sources = await services.catalog.listSources();
    expect(sources.find((s) => s.id === "src-beta")?.enabled).toBe(false);
    await services.catalog.setSourceEnabled("src-beta", true);
    const after = await services.catalog.listSources();
    expect(after.find((s) => s.id === "src-beta")?.enabled).toBe(true);
  });
});

describe("state reset", () => {
  it("restores seed after mutations", async () => {
    await services.library.removeFromLibrary("manga-003");
    await services.downloads.transition("task-003", "cancel");
    resetMockState();
    expect((await services.library.listManga()).map((m) => m.id)).toContain("manga-003");
    expect((await services.downloads.listTasks()).find((t) => t.id === "task-003")?.status).toBe("queued");
    expect(getMangaSummary("manga-003")).toBeDefined();
  });
});
