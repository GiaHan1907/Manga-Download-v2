import { useEffect, useState } from "react";
import { getAppInfo, getAppHealth, normalizeIpcError } from "../../shared/api/desktop";
import type { AppInfo, AppHealth, AppError } from "../../shared/api/contracts";
import ErrorState from "../../components/ui/ErrorState";
import StatusBadge from "../../components/ui/StatusBadge";

export type SystemStatusState =
  | { state: "loading" }
  | { state: "error"; error: AppError }
  | { state: "success"; info: AppInfo; health: AppHealth };

export default function SystemStatus() {
  const [status, setStatus] = useState<SystemStatusState>({ state: "loading" });

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
      <section className="panel system-status" aria-live="polite">
        <p className="muted">Đang kiểm tra hệ thống...</p>
      </section>
    );
  }

  if (status.state === "error") {
    return (
      <ErrorState
        title={`Lỗi: ${status.error.code}`}
        description={`${status.error.message}${status.error.retryable ? " Có thể thử lại." : ""}`}
      />
    );
  }

  return (
    <section className="panel system-status" aria-label="Trạng thái hệ thống">
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--space-3)" }}>
        <div>
          <h2 className="panel__title">Trạng thái nền tảng</h2>
          <p className="panel__description">IPC và SQLite từ Phase 01 vẫn đang hoạt động.</p>
        </div>
        <StatusBadge tone="success">Ready</StatusBadge>
      </div>
      <div className="system-status__grid">
        <StatusItem label="Tên" value={status.info.name} />
        <StatusItem label="Phiên bản" value={status.info.version} />
        <StatusItem label="Nền tảng" value={status.info.platform} />
        <StatusItem label="Trạng thái" value={status.health.status} />
        <StatusItem label="Cơ sở dữ liệu" value={status.health.database} />
        <StatusItem label="Phiên bản schema" value={String(status.health.schemaVersion)} />
      </div>
    </section>
  );
}

function StatusItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="system-status__item">
      <div className="system-status__label">{label}</div>
      <div className="system-status__value">{value}</div>
    </div>
  );
}
