import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, AppHealth, AppError } from "./contracts";

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_get_info");
}

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health_check");
}

export function normalizeIpcError(error: unknown): AppError {
  if (typeof error === "object" && error !== null) {
    const candidate = error as Record<string, unknown>;
    const code =
      candidate.code === "DATABASE_INIT_FAILED" || candidate.code === "INTERNAL_ERROR"
        ? candidate.code
        : "INTERNAL_ERROR";
    const message =
      typeof candidate.message === "string" ? candidate.message : "Unknown IPC error";
    const retryable = typeof candidate.retryable === "boolean" ? candidate.retryable : false;
    return { code, message, retryable };
  }
  return {
    code: "INTERNAL_ERROR",
    message: typeof error === "string" ? error : "Unknown IPC error",
    retryable: false,
  };
}
