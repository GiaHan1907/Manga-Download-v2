import { useEffect, useState } from "react";
import { getAppInfo, getAppHealth, normalizeIpcError } from "../../shared/api/desktop";
import type { AppInfo, AppHealth, AppError } from "../../shared/api/contracts";

type Status =
  | { state: "loading" }
  | { state: "error"; error: AppError }
  | { state: "success"; info: AppInfo; health: AppHealth };

export default function SystemStatus() {
  const [status, setStatus] = useState<Status>({ state: "loading" });

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const [info, health] = await Promise.all([getAppInfo(), getAppHealth()]);
        if (!cancelled) {
          setStatus({ state: "success", info, health });
        }
      } catch (err) {
        if (!cancelled) {
          setStatus({ state: "error", error: normalizeIpcError(err) });
        }
      }
    }

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  if (status.state === "loading") {
    return (
      <div style={{ color: "var(--color-text-muted)" }}>
        Đang kiểm tra hệ thống...
      </div>
    );
  }

  if (status.state === "error") {
    return (
      <div
        style={{
          padding: "1rem",
          borderRadius: "8px",
          backgroundColor: "var(--color-surface)",
          borderLeft: "4px solid var(--color-primary-hover)",
        }}
      >
        <strong style={{ color: "var(--color-text)" }}>Lỗi: {status.error.code}</strong>
        <p style={{ color: "var(--color-text-muted)", marginTop: "0.5rem" }}>
          {status.error.message}
        </p>
        {status.error.retryable && (
          <p style={{ color: "var(--color-text-muted)", fontSize: "0.875rem", marginTop: "0.25rem" }}>
            Có thể thử lại.
          </p>
        )}
      </div>
    );
  }

  return (
    <div
      style={{
        padding: "1rem",
        borderRadius: "8px",
        backgroundColor: "var(--color-surface)",
        display: "flex",
        flexDirection: "column",
        gap: "0.5rem",
      }}
    >
      <div>
        <span style={{ color: "var(--color-text-muted)" }}>Tên: </span>
        <span style={{ color: "var(--color-text)" }}>{status.info.name}</span>
      </div>
      <div>
        <span style={{ color: "var(--color-text-muted)" }}>Phiên bản: </span>
        <span style={{ color: "var(--color-text)" }}>{status.info.version}</span>
      </div>
      <div>
        <span style={{ color: "var(--color-text-muted)" }}>Nền tảng: </span>
        <span style={{ color: "var(--color-text)" }}>{status.info.platform}</span>
      </div>
      <div>
        <span style={{ color: "var(--color-text-muted)" }}>Trạng thái: </span>
        <span style={{ color: "var(--color-primary)" }}>{status.health.status}</span>
      </div>
      <div>
        <span style={{ color: "var(--color-text-muted)" }}>Cơ sở dữ liệu: </span>
        <span style={{ color: "var(--color-primary)" }}>{status.health.database}</span>
      </div>
    </div>
  );
}
