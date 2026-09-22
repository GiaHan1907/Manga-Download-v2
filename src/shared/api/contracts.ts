export type AppInfo = {
  name: string;
  version: string;
  platform: string;
};

export type AppHealth = {
  status: "ok";
  database: "ready";
};

export type AppError = {
  code: "DATABASE_INIT_FAILED" | "INTERNAL_ERROR";
  message: string;
  retryable: boolean;
};
