import type { CatalogService } from "./catalog.service";
import type { DownloadService } from "./download.service";
import type { LibraryService } from "./library.service";
import type { ReaderService } from "./reader.service";
import type { SettingsService } from "./settings.service";
import { localLibraryApi } from "./local-library.api";
import { settingsApi } from "./settings.api";
import { sourceApi } from "./source.api";
import { downloadsApi } from "./downloads.api";
import { libraryApi } from "./library.api";
import { readerApi } from "./reader.api";
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

// Demo sandbox (explicitly labeled in UI): catalog/downloads/reader remain
// fixture-backed until Phase 06–07. Settings is real since Phase 04 and
// discovery is real since Phase 05 (persisted/registry-backed via IPC).
// `localServices` is the production-mode boundary for data with a Rust backend.
export const services: AppServices = {
  catalog: mockCatalogService,
  library: mockLibraryService,
  downloads: mockDownloadService,
  reader: mockReaderService,
  settings: settingsApi,
};

export const localServices = {
  library: localLibraryApi,
  source: sourceApi,
  downloads: downloadsApi,
  libraryV2: libraryApi,
  reader: readerApi,
};

export const subscribeServices = subscribeMock;
export const getServicesVersion = getMockVersion;
export { resetMockState };
