import type { ReaderMode, ReadingDirection } from "../domain/reader";

export type OutputFormat = "cbz" | "folder" | "pdf";

export interface AppSettings {
  defaultReaderMode: ReaderMode;
  defaultReadingDirection: ReadingDirection;
  chapterConcurrency: number;
  imageConcurrency: number;
  outputFormat: OutputFormat;
  notificationsEnabled: boolean;
  closeToTray: boolean;
  watchEnabled: boolean;
  /// Page-download bandwidth cap in KB/s; 0 = unlimited.
  speedLimitKbps: number;
}

export const SETTINGS_LIMITS = {
  chapterConcurrency: { min: 1, max: 5 },
  imageConcurrency: { min: 1, max: 8 },
  /// KB/s across the whole engine; 0 = no cap (mirrors SPEED_LIMIT_MAX_KBPS).
  speedLimitKbps: { min: 0, max: 100_000 },
} as const;

export const DEFAULT_SETTINGS: AppSettings = {
  defaultReaderMode: "single",
  defaultReadingDirection: "rtl",
  chapterConcurrency: 2,
  imageConcurrency: 4,
  outputFormat: "cbz",
  notificationsEnabled: true,
  closeToTray: false,
  watchEnabled: true,
  speedLimitKbps: 0,
};

export interface SettingsService {
  get(): Promise<AppSettings>;
  save(settings: AppSettings): Promise<void>;
  reset(): Promise<void>;
}
