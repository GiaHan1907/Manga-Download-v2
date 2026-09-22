import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import Tooltip from "../../components/ui/Tooltip";
import { services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";

export default function SourcesPage() {
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
      <p className="muted">Bật/tắt nguồn ảnh hưởng trực tiếp tới bộ lọc trong Discover. Đây là trạng thái mock của phiên demo.</p>
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
      <div className="settings-actions">
        <Tooltip content="Chỉ áp dụng sau khi Source Engine (Phase 04+) được tích hợp — hiện chưa có adapter thật.">
          <Button disabled aria-label="Thêm nguồn — khả dụng sau Source Engine integration">
            Thêm nguồn (disabled)
          </Button>
        </Tooltip>
      </div>
    </div>
  );
}
