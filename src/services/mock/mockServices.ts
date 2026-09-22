import type { ChapterSummary, MangaSummary } from "../../domain/manga";
import { applyDownloadAction, type DemoDownloadTask } from "../../domain/download";
import type { ReaderPage, ReadingProgress } from "../../domain/reader";
import { DEMO_SOURCE_PATTERN, type CatalogService, type CatalogSource } from "../catalog.service";
import { InvalidTransitionError, type DownloadService, type QueueResult } from "../download.service";
import type { LibraryService } from "../library.service";
import type { ReaderService } from "../reader.service";
import { DEFAULT_SETTINGS, SETTINGS_LIMITS, type AppSettings, type SettingsService } from "../settings.service";
import {
  DEMO_IMPORT_URL,
  DEMO_SOURCES,
  getChapter,
  getMangaDetail,
  getMangaSummary,
  getChapters as fixtureChapters,
  getSamplePages,
  MANGA_SUMMARIES,
} from "./fixtures";
import { getMockState, updateMockState } from "./store";

function normalize(value: string): string {
  return value
    .toLowerCase()
    .replace(/đ/g, "d")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "");
}

export const mockCatalogService: CatalogService = {
  async search(query, sourceIds) {
    const normalized = normalize(query.trim());
    return MANGA_SUMMARIES.filter((manga) => {
      if (sourceIds.length > 0 && !sourceIds.includes(manga.sourceId)) {
        return false;
      }
      if (!normalized) {
        return true;
      }
      const haystack = normalize([manga.title, manga.author ?? "", manga.genres.join(" ")].join(" "));
      return haystack.includes(normalized);
    });
  },

  async resolveDemoUrl(url) {
    const match = DEMO_SOURCE_PATTERN.exec(url.trim());
    if (!match) {
      return null;
    }
    return getMangaSummary(match[1]) ?? null;
  },

  async getDemoExampleUrl() {
    return DEMO_IMPORT_URL;
  },

  async getManga(id) {
    return getMangaDetail(id) ?? null;
  },

  async getChapters(mangaId) {
    return fixtureChapters(mangaId);
  },

  async listSources() {
    const { enabledSourceIds } = getMockState();
    return DEMO_SOURCES.map<CatalogSource>((source) => ({
      ...source,
      enabled: enabledSourceIds.includes(source.id),
    }));
  },

  async setSourceEnabled(sourceId, enabled) {
    updateMockState((current) => ({
      ...current,
      enabledSourceIds: enabled
        ? [...new Set([...current.enabledSourceIds, sourceId])]
        : current.enabledSourceIds.filter((id) => id !== sourceId),
    }));
  },
};

export const mockLibraryService: LibraryService = {
  async listManga() {
    const { libraryIds } = getMockState();
    return libraryIds
      .map((id) => getMangaSummary(id))
      .filter((manga): manga is MangaSummary => manga !== undefined);
  },

  async listFavorites() {
    const { favoriteIds } = getMockState();
    return favoriteIds
      .map((id) => getMangaSummary(id))
      .filter((manga): manga is MangaSummary => manga !== undefined);
  },

  async isFavorite(mangaId) {
    return getMockState().favoriteIds.includes(mangaId);
  },

  async toggleFavorite(mangaId) {
    if (!getMangaSummary(mangaId)) {
      throw new Error(`Manga ${mangaId} không tồn tại trong catalog demo.`);
    }
    let nowFavorite = false;
    updateMockState((current) => {
      nowFavorite = !current.favoriteIds.includes(mangaId);
      return {
        ...current,
        favoriteIds: nowFavorite
          ? [...current.favoriteIds, mangaId]
          : current.favoriteIds.filter((id) => id !== mangaId),
      };
    });
    return nowFavorite;
  },

  async removeFromLibrary(mangaId) {
    updateMockState((current) => ({
      ...current,
      libraryIds: current.libraryIds.filter((id) => id !== mangaId),
    }));
  },
};

let taskCounter = 0;

export const mockDownloadService: DownloadService = {
  async listTasks() {
    return getMockState().tasks.map((task) => ({ ...task }));
  },

  async queueChapters(chapterIds) {
    const newTasks: DemoDownloadTask[] = [];
    let skipped = 0;
    for (const chapterId of chapterIds) {
      const chapter: ChapterSummary | undefined = getChapter(chapterId);
      if (!chapter) {
        skipped += 1;
        continue;
      }
      const existing = getMockState().tasks.find(
        (task) => task.chapterId === chapterId && task.status !== "cancelled",
      );
      if (existing) {
        skipped += 1;
        continue;
      }
      taskCounter += 1;
      newTasks.push({
        id: `demo-task-${taskCounter}`,
        mangaId: chapter.mangaId,
        chapterId: chapter.id,
        status: "queued",
        completedPages: 0,
        totalPages: chapter.pageCount ?? 0,
        createdAt: "2026-09-22T00:00:00.000Z",
      });
    }
    if (newTasks.length > 0) {
      updateMockState((current) => ({ ...current, tasks: [...current.tasks, ...newTasks] }));
    }
    const result: QueueResult = { added: newTasks.length, skipped };
    return result;
  },

  async transition(taskId, action) {
    const task = getMockState().tasks.find((candidate) => candidate.id === taskId);
    const nextStatus = task ? applyDownloadAction(task.status, action) : null;
    if (!task || !nextStatus) {
      throw new InvalidTransitionError(taskId, action);
    }
    updateMockState((current) => ({
      ...current,
      tasks: current.tasks.map((candidate) => {
        if (candidate.id !== taskId) {
          return candidate;
        }
        const updated: DemoDownloadTask = { ...candidate, status: nextStatus };
        if (action === "retry") {
          updated.completedPages = 0;
        }
        return updated;
      }),
    }));
  },
};

export const mockReaderService: ReaderService = {
  async getPages(chapterId): Promise<ReaderPage[]> {
    return getSamplePages(chapterId);
  },

  async getProgress(mangaId): Promise<ReadingProgress | null> {
    return getMockState().readingProgress[mangaId] ?? null;
  },

  async listProgress(): Promise<ReadingProgress[]> {
    return Object.values(getMockState().readingProgress).map((progress) => ({ ...progress }));
  },

  async setProgress(progress) {
    const manga = getMangaSummary(progress.mangaId);
    const chapter = getChapter(progress.chapterId);
    if (!manga || !chapter || chapter.mangaId !== progress.mangaId) {
      throw new Error("Không thể lưu tiến độ cho dữ liệu mẫu không khớp nhau.");
    }
    updateMockState((current) => ({
      ...current,
      readingProgress: { ...current.readingProgress, [progress.mangaId]: { ...progress } },
    }));
  },
};

function validateSettings(settings: AppSettings): AppSettings {
  const { chapterConcurrency, imageConcurrency } = SETTINGS_LIMITS;
  if (
    !Number.isInteger(settings.chapterConcurrency) ||
    settings.chapterConcurrency < chapterConcurrency.min ||
    settings.chapterConcurrency > chapterConcurrency.max
  ) {
    throw new Error(`Số chương đồng thời phải là số nguyên từ ${chapterConcurrency.min} đến ${chapterConcurrency.max}.`);
  }
  if (
    !Number.isInteger(settings.imageConcurrency) ||
    settings.imageConcurrency < imageConcurrency.min ||
    settings.imageConcurrency > imageConcurrency.max
  ) {
    throw new Error(`Số ảnh đồng thời phải là số nguyên từ ${imageConcurrency.min} đến ${imageConcurrency.max}.`);
  }
  if (!["single", "double", "vertical"].includes(settings.defaultReaderMode)) {
    throw new Error("Chế độ đọc không hợp lệ.");
  }
  if (!["ltr", "rtl"].includes(settings.defaultReadingDirection)) {
    throw new Error("Hướng đọc không hợp lệ.");
  }
  if (!["cbz", "folder", "pdf"].includes(settings.outputFormat)) {
    throw new Error("Định dạng xuất không hợp lệ.");
  }
  return { ...settings };
}

export const mockSettingsService: SettingsService = {
  async get() {
    return { ...getMockState().settings };
  },

  async save(settings) {
    const validated = validateSettings(settings);
    updateMockState((current) => ({ ...current, settings: validated }));
  },

  async reset() {
    updateMockState((current) => ({ ...current, settings: { ...DEFAULT_SETTINGS } }));
  },
};
