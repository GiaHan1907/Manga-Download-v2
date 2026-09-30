import { Settings2 } from "lucide-react";
import { useState } from "react";
import Button from "../../components/ui/Button";
import Skeleton from "../../components/ui/Skeleton";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import type { SourceCapability, SourceViewDto } from "../../types/source";

/// What each capability buys the user, in the app's own words.
const CAPABILITY_LABEL: Record<SourceCapability, string> = {
  search: "tìm kiếm",
  resolveUrl: "nhập URL",
  details: "chi tiết",
  pages: "đọc",
};

/// Every registered source with its real enable state. No counts are shown:
/// the sources publish no per-genre or per-status totals, so a number here
/// would be invented.
function SourceRow({ source, onToggled }: { source: SourceViewDto; onToggled: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function toggle(enabled: boolean) {
    setBusy(true);
    setError(null);
    try {
      await localServices.source.setSourceEnabled({ sourceId: source.id, enabled });
      bumpDomain("sources");
      onToggled();
    } catch (caught: unknown) {
      setError(normalizeIpcError(caught).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <li className="discover-source">
      <label className="discover-source__row">
        <span className="discover-source__logo" aria-hidden="true">
          {source.displayName.trim().charAt(0).toUpperCase()}
        </span>
        <span className="discover-source__text">
          <span className="discover-source__name">{source.displayName}</span>
          <span className="muted discover-source__caps">
            {source.capabilities.length > 0
              ? source.capabilities.map((c) => CAPABILITY_LABEL[c]).join(" · ")
              : "không có khả năng nào"}
          </span>
        </span>
        <input
          type="checkbox"
          className="discover-source__toggle"
          checked={source.enabled}
          disabled={busy}
          onChange={(event) => void toggle(event.target.checked)}
          aria-label={`Kích hoạt ${source.displayName}`}
        />
      </label>
      {error && (
        <span className="queue-error" role="alert">
          {error}
        </span>
      )}
    </li>
  );
}

export default function DiscoverSourcesPanel() {
  const sourcesVersion = useDomainVersion("sources");
  const sources = useAsyncData<SourceViewDto[]>(
    () => localServices.source.listSources(),
    [sourcesVersion],
  );

  return (
    <section className="panel discover-sources" aria-label="Nguồn truyện được hỗ trợ">
      <h2 className="panel__title">Nguồn truyện được hỗ trợ</h2>
      {sources.state === "loading" && <Skeleton height="8rem" label="Đang tải danh sách nguồn" />}
      {sources.state === "error" && (
        <p className="queue-error" role="alert">
          {normalizeIpcError(sources.error).message}
        </p>
      )}
      {sources.state === "success" && (
        <>
          <ul className="discover-sources__list">
            {sources.data.map((source) => (
              <SourceRow key={source.id} source={source} onToggled={() => {}} />
            ))}
          </ul>
          <Button to="/settings/sources" variant="secondary" className="discover-sources__manage">
            <Settings2 size={16} aria-hidden="true" />
            Quản lý nguồn truyện
          </Button>
        </>
      )}
    </section>
  );
}
