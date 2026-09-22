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
  | "INTERNAL_ERROR";

export type AppError = {
  code: AppErrorCode;
  message: string;
  retryable: boolean;
};
