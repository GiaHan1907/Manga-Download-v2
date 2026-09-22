import type { DemoDownloadTask } from "../../domain/download";
import type { ReadingProgress } from "../../domain/reader";
import type { AppSettings } from "../settings.service";
import { DEFAULT_SETTINGS } from "../settings.service";
import { SEED_FAVORITE_IDS, SEED_LIBRARY_IDS, SEED_TASKS } from "./fixtures";

export interface MockState {
  libraryIds: string[];
  favoriteIds: string[];
  tasks: DemoDownloadTask[];
  readingProgress: Record<string, ReadingProgress>;
  enabledSourceIds: string[];
  settings: AppSettings;
}

function seedState(): MockState {
  return {
    libraryIds: [...SEED_LIBRARY_IDS],
    favoriteIds: [...SEED_FAVORITE_IDS],
    tasks: SEED_TASKS.map((task) => ({ ...task })),
    readingProgress: {},
    enabledSourceIds: ["src-alpha", "src-beta"],
    settings: { ...DEFAULT_SETTINGS },
  };
}

let state: MockState = seedState();
let version = 0;
const listeners = new Set<() => void>();

export function getMockState(): MockState {
  return state;
}

export function getMockVersion(): number {
  return version;
}

export function subscribeMock(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function updateMockState(updater: (current: MockState) => MockState): void {
  state = updater(state);
  version += 1;
  for (const listener of listeners) {
    listener();
  }
}

// Test/dev helper only — deliberately not exposed as an end-user button.
export function resetMockState(): void {
  state = seedState();
  version += 1;
  for (const listener of listeners) {
    listener();
  }
}
