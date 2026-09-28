import { useState } from "react";
import { Link } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useDomainVersion } from "../../shared/hooks/useInvalidation";
import type {
  RemoteMangaSummaryDto,
  SearchResultDto,
  SourceViewDto,
} from "../../types/source";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import { useDebouncedValue } from "./hooks/useDebouncedValue";

// Isolated so the cover hook runs per card (React rules of hooks forbid
// calling it inline inside the results .map).
function LiveSearchCard({ item }: { item: RemoteMangaSummaryDto }) {
  const coverUrl = useCoverUrl(item.sourceId, item.sourceMangaId, item.coverKey);
  return (
    <Link
      to={`/remote/${encodeURIComponent(item.sourceId)}/${encodeURIComponent(item.sourceMangaId)}`}
      className="card-link"
      aria-label={`Mở xem trước ${item.title}`}
    >
      <MangaCard
        title={item.title}
        subtitle={`Nguồn: ${item.sourceDisplayName}${item.author ? ` · ${item.author}` : ""}`}
        status={item.status}
        coverUrl={coverUrl ?? undefined}
      />
    </Link>
  );
}

/// Append one page of results to what the user already sees, dropping items
/// already present (adapters dedupe within a page, not across pages).
function mergeUnique(
  prev: RemoteMangaSummaryDto[],
  next: RemoteMangaSummaryDto[],
): RemoteMangaSummaryDto[] {
  const seen = new Set(prev.map((item) => `${item.sourceId}:${item.sourceMangaId}`));
  return [...prev, ...next.filter((item) => !seen.has(`${item.sourceId}:${item.sourceMangaId}`))];
}

/// Pages loaded via "Xem thêm" plus their transient UI state. Keyed by the
/// search identity so a query/filter change drops stale pages immediately.
type MoreState = {
  key: string;
  pages: SearchResultDto[];
  loading: boolean;
  error: string | null;
};

const EMPTY_MORE: MoreState = { key: "", pages: [], loading: false, error: null };

export default function LiveSearchPanel({ initialQuery = "" }: { initialQuery?: string }) {
  const [query, setQuery] = useState(initialQuery);
  const [selected, setSelected] = useState<string[]>([]);
  const [reload, setReload] = useState(0);
  const sourcesVersion = useDomainVersion("sources");
  const debounced = useDebouncedValue(query.trim(), 300);

  const sources = useAsyncData<SourceViewDto[]>(() => localServices.source.listSources(), [
    reload,
    sourcesVersion,
  ]);

  const searchable =
    sources.state === "success"
      ? sources.data.filter((source) => source.enabled && source.capabilities.includes("search"))
      : [];
  const effectiveIds =
    selected.length > 0 ? selected.filter((id) => searchable.some((s) => s.id === id)) : undefined;

  // One search identity: query + source filter + manual reload. Page 1 is
  // always refetched on change; extra pages are dropped render-side (the
  // React derived-state pattern — no effect, no stale-frame flash).
  const searchKey = ["q", debounced, effectiveIds?.join(",") ?? "all", "r", reload].join("|");
  const [more, setMore] = useState<MoreState>(EMPTY_MORE);
  if (more.key !== searchKey) {
    setMore({ key: searchKey, pages: [], loading: false, error: null });
  }

  const results = useAsyncData<SearchResultDto | null>(
    () =>
      debounced.length < 2
        ? Promise.resolve(null)
        : localServices.source.search({ query: debounced, sourceIds: effectiveIds, page: 1 }),
    // effectiveIds key: disabling/removing a source must invalidate rendered
    // results that were gathered from sources which are no longer searchable.
    [debounced, selected.join(","), effectiveIds?.join(",") ?? "all", reload],
  );

  // Everything below is derived during render — accumulating in an effect
  // would only add cascading renders.
  const firstPage = results.state === "success" ? results.data : null;
  const allPages: SearchResultDto[] = firstPage ? [firstPage, ...more.pages] : [];
  const items = allPages.reduce(
    (acc, page) => (page === firstPage ? page.items : mergeUnique(acc, page.items)),
    [] as RemoteMangaSummaryDto[],
  );
  const sourceErrors = allPages.length > 0 ? allPages[allPages.length - 1].sourceErrors : [];
  const hasMore = allPages.length > 0 ? allPages[allPages.length - 1].hasMore : false;

  async function loadMore() {
    if (more.loading || debounced.length < 2) {
      return;
    }
    const key = searchKey;
    const nextPage = more.pages.length + 2;
    setMore((current) => (current.key === key ? { ...current, loading: true, error: null } : current));
    try {
      const page = await localServices.source.search({
        query: debounced,
        sourceIds: effectiveIds,
        page: nextPage,
      });
      // A stale response (query changed mid-flight) must not leak in.
      setMore((current) =>
        current.key === key ? { ...current, pages: [...current.pages, page], loading: false } : current,
      );
    } catch (error: unknown) {
      setMore((current) =>
        current.key === key
          ? { ...current, loading: false, error: normalizeIpcError(error).message }
          : current,
      );
    }
  }

  function toggleSource(sourceId: string) {
    setSelected((current) =>
      current.includes(sourceId) ? current.filter((id) => id !== sourceId) : [...current, sourceId],
    );
  }

  if (sources.state === "loading") {
    return <Skeleton height="10rem" label="Đang tải danh sách nguồn" />;
  }
  if (sources.state === "error") {
    const error = normalizeIpcError(sources.error);
    return (
      <ErrorState
        title="Không đọc được danh sách nguồn"
        description={`${error.message} (mã: ${error.code})`}
        action={<Button onClick={() => setReload((n) => n + 1)}>Thử lại</Button>}
      />
    );
  }

  const failedSources = sourceErrors;

  return (
    <div className="page-stack">
      <div className="discover-controls">
        <Input
          label="Tìm kiếm"
          type="search"
          placeholder="Nhập tên truyện (tối thiểu 2 ký tự)..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <fieldset className="source-filter" aria-label="Lọc theo nguồn">
          <legend>Nguồn</legend>
          {searchable.length === 0 && (
            <p className="muted">Chưa có nguồn nào bật hỗ trợ tìm kiếm. Đăng ký trong Cài đặt → Nguồn.</p>
          )}
          {searchable.map((source) => (
            <label key={source.id} className="source-filter__option">
              <input
                type="checkbox"
                checked={selected.includes(source.id)}
                onChange={() => toggleSource(source.id)}
              />
              {source.displayName}
            </label>
          ))}
        </fieldset>
      </div>

      {failedSources.length > 0 && (
        <ul className="source-errors" role="status" aria-label="Một số nguồn gặp lỗi">
          {failedSources.map((failure) => (
            <li key={`${failure.sourceId}:${failure.code}`}>
              <StatusBadge tone="error">{failure.code}</StatusBadge>{" "}
              {sources.data.find((source) => source.id === failure.sourceId)?.displayName ??
                failure.sourceId}
              : {failure.message}
            </li>
          ))}
        </ul>
      )}

      {results.state === "loading" && items.length === 0 && (
        <div className="card-grid" aria-busy="true">
          <Skeleton height="18rem" />
          <Skeleton height="18rem" />
        </div>
      )}
      {results.state === "error" && (
        <ErrorState
          title="Tìm kiếm thất bại"
          description={normalizeIpcError(results.error).message}
          action={<Button onClick={() => setReload((n) => n + 1)}>Thử lại</Button>}
        />
      )}
      {results.state === "success" && results.data === null && (
        <EmptyState
          title="Chưa có truy vấn"
          description="Nhập ít nhất 2 ký tự để tìm."
        />
      )}
      {results.state === "success" &&
        results.data !== null &&
        items.length === 0 &&
        failedSources.length === 0 && (
          <EmptyState
            title="Không có kết quả"
            description="Các nguồn đang bật đều trả về rỗng. Thử từ khóa khác hoặc bỏ bớt bộ lọc nguồn."
            action={<Button onClick={() => setQuery("")}>Xóa từ khóa</Button>}
          />
        )}
      {items.length > 0 && (
        <div className="card-grid">
          {items.map((item) => (
            <LiveSearchCard
              key={`${item.sourceId}:${item.sourceMangaId}`}
              item={item}
            />
          ))}
        </div>
      )}
      {items.length > 0 && hasMore && (
        <div className="settings-actions">
          <Button variant="secondary" disabled={more.loading} onClick={() => void loadMore()}>
            {more.loading ? "Đang tải trang tiếp…" : "Xem thêm"}
          </Button>
        </div>
      )}
      {more.error && (
        <p className="queue-error" role="alert">
          Không tải được trang tiếp theo ({more.error}) — bấm "Xem thêm" để thử lại.
        </p>
      )}
    </div>
  );
}
