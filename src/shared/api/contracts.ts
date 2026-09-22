export type AppInfo = {
  name: string;
  version: string;
  platform: string;
};

export type AppHealth = {
  status: "ok";
  database: "ready";
  schemaVersion: number;
};

export type AppErrorCode =
  | "DATABASE_INIT_FAILED"
  | "VALIDATION_ERROR"
  | "NOT_FOUND"
  | "DB_CONFLICT"
  | "UNSUPPORTED_IN_THIS_PHASE"
  | "INTERNAL_ERROR"
  | "INVALID_URL"
  | "UNSUPPORTED_SOURCE"
  | "SOURCE_DISABLED"
  | "UNSUPPORTED_OPERATION"
  | "SOURCE_NOT_FOUND"
  | "SOURCE_FORBIDDEN"
  | "SOURCE_RATE_LIMITED"
  | "SOURCE_NETWORK"
  | "SOURCE_PARSE_ERROR"
  | "SOURCE_UPSTREAM"
  | "LIBRARY_ITEM_NOT_FOUND"
  | "LOCAL_FILE_MISSING"
  | "LOCAL_FILE_CORRUPTED"
  | "CBZ_INVALID"
  | "UNSUPPORTED_IMAGE_FORMAT"
  | "PAGE_RESOURCE_NOT_FOUND"
  | "DELETE_FAILED"
  | "STORAGE_OUTSIDE_LIBRARY_ROOT";

export type AppError = {
  code: AppErrorCode;
  message: string;
  retryable: boolean;
};
