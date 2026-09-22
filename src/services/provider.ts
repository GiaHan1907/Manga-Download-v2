import type { CatalogService } from "./catalog.service";
import type { DownloadService } from "./download.service";
import type { LibraryService } from "./library.service";
import type { ReaderService } from "./reader.service";
import type { SettingsService } from "./settings.service";
import { localLibraryApi } from "./local-library.api";
import { settingsApi } from "./settings.api";
import {
  mockCatalogService,
  mockDownloadService,
  mockLibraryService,
  mockReaderService,
} from "./mock/mockServices";
import { getMockVersion, resetMockState, subscribeMock } from "./mock/store";

export interface AppServices {
  catalog: CatalogService;
  library: LibraryService;
  downloads: DownloadService;
  reader: ReaderService;
  settings: SettingsService;
}

// Demo sandbox (explicitly labeled in UI): catalog/library/downloads/reader
// remain fixture-backed until Phase 05–07. Settings is REAL since Phase 04
// (persisted in SQLite via IPC). `localServices` is the production-mode
// boundary for data that already has a Rust backend.
export const services: AppServices = {
  catalog: mockCatalogService,
  library: mockLibraryService,
  downloads: mockDownloadService,
  reader: mockReaderService,
  settings: settingsApi,
};

export const localServices = {
  library: localLibraryApi,
};

export const subscribeServices = subscribeMock;
export const getServicesVersion = getMockVersion;
export { resetMockState };
