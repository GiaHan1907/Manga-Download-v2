import type { CatalogService } from "./catalog.service";
import type { DownloadService } from "./download.service";
import type { LibraryService } from "./library.service";
import type { ReaderService } from "./reader.service";
import type { SettingsService } from "./settings.service";
import {
  mockCatalogService,
  mockDownloadService,
  mockLibraryService,
  mockReaderService,
  mockSettingsService,
} from "./mock/mockServices";
import { getMockVersion, resetMockState, subscribeMock } from "./mock/store";

export interface AppServices {
  catalog: CatalogService;
  library: LibraryService;
  downloads: DownloadService;
  reader: ReaderService;
  settings: SettingsService;
}

// Phase 03 wires mock implementations. Phase 04+ replaces these with
// IPC-backed services behind the same interfaces — screens stay untouched.
export const services: AppServices = {
  catalog: mockCatalogService,
  library: mockLibraryService,
  downloads: mockDownloadService,
  reader: mockReaderService,
  settings: mockSettingsService,
};

export const subscribeServices = subscribeMock;
export const getServicesVersion = getMockVersion;
export { resetMockState };
