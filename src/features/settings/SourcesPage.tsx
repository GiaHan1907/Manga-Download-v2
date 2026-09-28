import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import Tooltip from "../../components/ui/Tooltip";
import { localServices, services } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain } from "../../shared/hooks/useInvalidation";
import { useServicesVersion } from "../../shared/hooks/useServices";
import type { SourceViewDto } from "../../types/source";

function RealSources() {
  const [reload, setReload] = useState(0);
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [imageHostsText, setImageHostsText] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const sources = useAsyncData<SourceViewDto[]>(() => localServices.source.listSources(), [reload]);

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setActionError(null);
    try {
      await action();
      // Cross-screen: Discover's source list/filters follow registry changes.
      bumpDomain("sources");
      setReload((n) => n + 1);
    } catch (error: unknown) {
      setActionError(normalizeIpcError(error).message);
    } finally {
      setBusy(false);
    }
  }

  if (sources.state === "loading") {
    return <Skeleton height="10rem" label="Đang tải nguồn" />;
  }
  if (sources.state === "error") {
    const error = normalizeIpcError(sources.error);
    return (
      <ErrorState
        title="Không đọc được registry nguồn"
        description={`${error.message} (mã: ${error.code})`}
        action={<Button onClick={() => setReload((n) => n + 1)}>Thử lại</Button>}
      />
    );
  }

  return (
    <div className="page-stack">
      <div className="sources-list">
        {sources.data.map((source) => (
          <div key={source.id} className="source-row">
            <div>
              <h3>{source.displayName}</h3>
              <p className="muted">{source.baseUrl ?? source.id}</p>
              {source.imageHosts && (
                <p className="muted">Host ảnh: {source.imageHosts}</p>
              )}
              <div className="chip-row">
                <StatusBadge tone={source.builtin ? "neutral" : "success"}>
                  {source.builtin ? "Tích hợp sẵn" : "Do người dùng thêm"}
                </StatusBadge>
                {source.capabilities.map((capability) => (
                  <StatusBadge key={capability} tone="neutral">
                    {capability}
                  </StatusBadge>
                ))}
                {source.enabled ? (
                  <StatusBadge tone="warning">bật</StatusBadge>
                ) : (
                  <StatusBadge tone="neutral">tắt</StatusBadge>
                )}
              </div>
              {source.attribution && <p className="muted">{source.attribution}</p>}
            </div>
            <div className="chip-row">
              <Button
                variant={source.enabled ? "secondary" : "primary"}
                disabled={busy}
                onClick={() =>
                  void run(() =>
                    localServices.source.setSourceEnabled({
                      sourceId: source.id,
                      enabled: !source.enabled,
                    }),
                  )
                }
              >
                {source.enabled ? "Tắt" : "Bật"}
              </Button>
              {source.builtin ? (
                <Tooltip
                  content="Nguồn tích hợp sẵn của app — không thể xoá, chỉ có thể tắt."
                  focusableWhenDisabled
                >
                  <Button disabled aria-label={`${source.displayName}: không xoá được (nguồn tích hợp)`}>
                    Xoá
                  </Button>
                </Tooltip>
              ) : (
                <Button
                  variant="secondary"
                  disabled={busy}
                  onClick={() => void run(() => localServices.source.removeSource(source.id))}
                >
                  Xoá nguồn
                </Button>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="discover-controls">
        <Input
          label="Tên nguồn mới"
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="VD: Truyen Tuoi Tho"
        />
        <Input
          label="URL gốc (HTTPS, chỉ host đã đăng ký mới được gọi)"
          value={baseUrl}
          onChange={(event) => setBaseUrl(event.target.value)}
          placeholder="https://vd-site-madara.com/"
        />
        <Input
          label="Host ảnh phụ trợ (tuỳ chọn, tối đa 4, phân cách bằng dấu phẩy)"
          value={imageHostsText}
          onChange={(event) => setImageHostsText(event.target.value)}
          placeholder="VD: img-cdn.example.com"
        />
      </div>
      {actionError && <ErrorState title="Thao tác với nguồn thất bại" description={actionError} />}
      <div className="settings-actions">
        <Button
          variant="primary"
          disabled={busy || name.trim() === "" || baseUrl.trim() === ""}
          onClick={() => {
            const hosts = imageHostsText
              .split(",")
              .map((entry) => entry.trim())
              .filter((entry) => entry !== "");
            void run(() =>
              localServices.source.addSource({
                displayName: name.trim(),
                baseUrl: baseUrl.trim(),
                imageHosts: hosts.length > 0 ? hosts : undefined,
              }),
            ).then(() => setImageHostsText(""));
          }}
        >
          {busy ? "Đang xử lý..." : "Thêm nguồn"}
        </Button>
      </div>
    </div>
  );
}

function DemoSources() {
  const version = useServicesVersion();
  const sources = useAsyncData(() => services.catalog.listSources(), [version]);

  if (sources.state === "loading") {
    return <Skeleton height="10rem" label="Đang tải danh sách nguồn demo" />;
  }
  if (sources.state === "error") {
    return <ErrorState title="Không tải được danh sách nguồn" description="Mock catalog service lỗi. Reload để thử lại." />;
  }

  return (
    <div className="page-stack">
      <div className="sources-list">
        {sources.data.map((source) => (
          <div key={source.id} className="source-row">
            <div>
              <h3>{source.label}</h3>
              <div className="chip-row">
                <StatusBadge tone="warning">Demo source</StatusBadge>
                <StatusBadge tone="neutral">fixture search</StatusBadge>
                <StatusBadge tone="neutral">local covers</StatusBadge>
                {source.enabled ? <StatusBadge tone="success">enabled</StatusBadge> : <StatusBadge tone="neutral">disabled</StatusBadge>}
              </div>
            </div>
            <Button
              variant={source.enabled ? "secondary" : "primary"}
              onClick={() => void services.catalog.setSourceEnabled(source.id, !source.enabled)}
            >
              {source.enabled ? "Tắt nguồn" : "Bật nguồn"}
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function SourcesPage() {
  return (
    <div className="page-stack">
      <h2>Nguồn truyện</h2>
      <RealSources />
      {/* Plan §2.7: the mock sandbox is dev-build only — never rendered in a
          production app where it could be mistaken for real sources. */}
      {import.meta.env.DEV && (
        <>
          <h2>Demo sandbox (Phase 03)</h2>
          <DemoSources />
        </>
      )}
    </div>
  );
}
