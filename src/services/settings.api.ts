import * as desktop from "../shared/api/desktop";
import type { AppSettingsDto, SettingsPatchDto } from "../types/backend";
import { SETTINGS_LIMITS, type AppSettings, type SettingsService } from "./settings.service";
import { rethrowIpc } from "../shared/api/errors";

function toDto(settings: AppSettings): AppSettingsDto {
  return {
    defaultReaderMode: settings.defaultReaderMode,
    defaultReadingDirection: settings.defaultReadingDirection,
    chapterConcurrency: settings.chapterConcurrency,
    imageConcurrency: settings.imageConcurrency,
    outputFormat: settings.outputFormat,
    notificationsEnabled: settings.notificationsEnabled,
    closeToTray: settings.closeToTray,
    watchEnabled: settings.watchEnabled,
    speedLimitKbps: settings.speedLimitKbps,
  };
}

/// Phase 04: settings persist through SQLite via IPC (survive app restart).
/// Client-side bound checks mirror the Rust validation for snappy UX; the
/// backend remains the authority.
export const settingsApi: SettingsService = {
  async get(): Promise<AppSettings> {
    try {
      return await desktop.getBackendSettings();
    } catch (err) {
      rethrowIpc(err);
    }
  },

  async save(settings: AppSettings): Promise<void> {
    const { chapterConcurrency, imageConcurrency, speedLimitKbps } = SETTINGS_LIMITS;
    if (
      !Number.isInteger(settings.chapterConcurrency) ||
      settings.chapterConcurrency < chapterConcurrency.min ||
      settings.chapterConcurrency > chapterConcurrency.max
    ) {
      throw new Error(
        `Số chương đồng thời phải là số nguyên từ ${chapterConcurrency.min} đến ${chapterConcurrency.max}.`,
      );
    }
    if (
      !Number.isInteger(settings.imageConcurrency) ||
      settings.imageConcurrency < imageConcurrency.min ||
      settings.imageConcurrency > imageConcurrency.max
    ) {
      throw new Error(
        `Số ảnh đồng thời phải là số nguyên từ ${imageConcurrency.min} đến ${imageConcurrency.max}.`,
      );
    }
    if (
      !Number.isInteger(settings.speedLimitKbps) ||
      settings.speedLimitKbps < speedLimitKbps.min ||
      settings.speedLimitKbps > speedLimitKbps.max
    ) {
      throw new Error(
        `Giới hạn tốc độ phải là số nguyên từ ${speedLimitKbps.min} (không giới hạn) đến ${speedLimitKbps.max} KB/s.`,
      );
    }
    try {
      const patch: SettingsPatchDto = toDto(settings);
      await desktop.updateBackendSettings(patch);
    } catch (err) {
      rethrowIpc(err);
    }
  },

  async reset(): Promise<void> {
    try {
      await desktop.resetBackendSettings();
    } catch (err) {
      rethrowIpc(err);
    }
  },
};
