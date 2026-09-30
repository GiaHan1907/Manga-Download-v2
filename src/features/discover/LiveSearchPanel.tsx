import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { History, Search, SlidersHorizontal, X } from "lucide-react";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import StatusBadge from "../../components/ui/StatusBadge";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import type { RecentSearchDto } from "../../types/backend";
import type {
  RemoteMangaSummaryDto,
  SearchResultDto,
  SourceViewDto,
} from "../../types/source";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";

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

/// Remembered across screen switches: query + source filter only (page 1 is
/// always refetched fresh). Deep links via ?q= win over the stored value.
const RESTORE_KEY = "m1989.discover.search.v1";

function readStoredSearch(): { query: string; selected: string[] } | null {
  try {
    const raw = sessionStorage.getItem(RESTORE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return null;
    const { query, selected } = parsed as { query?: unknown; selected?: unknown };
    if (typeof query !== "string" || !Array.isArray(selected)) return null;
    return { query, selected: selected.filter((x): x is string => typeof x === "string") };
  } catch {
    return null;
  }
}

export default function LiveSearchPanel({ initialQuery = "" }: { initialQuery?: string }) {
  const [stored] = useState(() => (initialQuery ? null : readStoredSearch()));
  const [query, setQuery] = useState(initialQuery || stored?.query || "");
  const [sourceFilter, setSourceFilter] = useState<string>(stored?.selected[0] ?? "all");
  const [reload, setReload] = useState(0);
  const sourcesVersion = useDomainVersion("sources");
  // The term actually searched. Typing settles after 300ms; pressing
  // "Tìm kiếm" flushes it immediately so the button can never no-op mid-debounce.
  const [term, setTerm] = useState(() => query.trim());
  useEffect(() => {
    const timer = window.setTimeout(() => setTerm(query.trim()), 300);
    return () => window.clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    try {
      sessionStorage.setItem(
        RESTORE_KEY,
        JSON.stringify({ query: term, selected: sourceFilter === "all" ? [] : [sourceFilter] }),
      );
    } catch {
      /* private-mode quota — memory aid, not function-critical */
    }
  }, [term, sourceFilter]);

  const sources = useAsyncData<SourceViewDto[]>(() => localServices.source.listSources(), [
    reload,
    sourcesVersion,
  ]);

  // Recent queries are committed searches (submit / chip / Enter), not every
  // keystroke, so the list stays what the user actually looked for.
  const historyVersion = useDomainVersion("history");
  const history = useAsyncData<RecentSearchDto[]>(
    () => localServices.history.listSearches(),
    [historyVersion],
  );
  const [historyError, setHistoryError] = useState<string | null>(null);

  /// `raw` lets a chip search its own text in the same tick it is clicked —
  /// reading `query` here would still see the pre-click value.
  function commit(raw?: string) {
    const value = (raw ?? query).trim();
    if (value.length < 2) {
      return;
    }
    setTerm(value);
    setQuery(value);
    setReload((count) => count + 1);
    void localServices.history
      .recordSearch(value)
      .then(() => {
        setHistoryError(null);
        bumpDomain("history");
      })
      .catch((error: unknown) => setHistoryError(normalizeIpcError(error).message));
  }

  async function dropSearch(entry: string) {
    try {
      await localServices.history.removeSearch(entry);
      setHistoryError(null);
      bumpDomain("history");
    } catch (error: unknown) {
      setHistoryError(normalizeIpcError(error).message);
    }
  }

  async function clearSearches() {
    try {
      await localServices.history.clearSearches();
      setHistoryError(null);
      bumpDomain("history");
    } catch (error: unknown) {
      setHistoryError(normalizeIpcError(error).message);
    }
  }

  const searchable =
    sources.state === "success"
      ? sources.data.filter((source) => source.enabled && source.capabilities.includes("search"))
      : [];
  // A source that got disabled or removed mid-session falls back to "all"
  // rather than silently searching nothing.
  const effectiveIds =
    sourceFilter !== "all" && searchable.some((source) => source.id === sourceFilter)
      ? [sourceFilter]
      : undefined;

  // One search identity: query + source filter + manual reload. Page 1 is
  // always refetched on change; extra pages are dropped render-side (the
  // React derived-state pattern — no effect, no stale-frame flash).
  const searchKey = ["q", term, effectiveIds?.join(",") ?? "all", "r", reload].join("|");
  const [more, setMore] = useState<MoreState>(EMPTY_MORE);
  if (more.key !== searchKey) {
    setMore({ key: searchKey, pages: [], loading: false, error: null });
  }

  const results = useAsyncData<SearchResultDto | null>(
    () =>
      term.length < 2
        ? Promise.resolve(null)
        : localServices.source.search({ query: term, sourceIds: effectiveIds, page: 1 }),
    // effectiveIds key: disabling/removing a source must invalidate rendered
    // results that were gathered from sources which are no longer searchable.
    [term, sourceFilter, effectiveIds?.join(",") ?? "all", reload],
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
    if (more.loading || term.length < 2) {
      return;
    }
    const key = searchKey;
    const nextPage = more.pages.length + 2;
    setMore((current) => (current.key === key ? { ...current, loading: true, error: null } : current));
    try {
      const page = await localServices.source.search({
        query: term,
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
      <form
        className="discover-search"
        onSubmit={(event) => {
          event.preventDefault();
          commit();
        }}
      >
        <Search size={16} aria-hidden="true" />
        <input
          className="discover-search__input"
          aria-label="Tìm kiếm truyện"
          type="search"
          placeholder="Nhập tên truyện, tác giả, hoặc từ khóa (tối thiểu 2 ký tự)..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <Button variant="primary" type="submit" disabled={query.trim().length < 2}>
          Tìm kiếm
        </Button>
      </form>

      <div className="discover-filters">
        <label className="filter-pill">
          <SlidersHorizontal size={13} aria-hidden="true" />
          <span>Nguồn:</span>
          <select
            value={sourceFilter}
            disabled={searchable.length === 0}
            onChange={(event) => setSourceFilter(event.target.value)}
          >
            <option value="all">Tất cả</option>
            {searchable.map((source) => (
              <option key={source.id} value={source.id}>
                {source.displayName}
              </option>
            ))}
          </select>
        </label>
        {searchable.length === 0 && (
          <p className="muted">Chưa có nguồn nào bật hỗ trợ tìm kiếm. Đăng ký trong Cài đặt → Nguồn.</p>
        )}
      </div>

      {history.state === "success" && history.data.length > 0 && (
        <div className="discover-recent" aria-label="Tìm kiếm gần đây">
          <span className="muted discover-recent__label">
            <History size={13} aria-hidden="true" /> Tìm kiếm gần đây
          </span>
          {history.data.map((entry) => (
            <span key={entry.query} className="recent-chip">
              <button
                type="button"
                className="recent-chip__text"
                onClick={() => commit(entry.query)}
              >
                {entry.query}
              </button>
              <button
                type="button"
                className="recent-chip__remove"
                aria-label={`Xóa ${entry.query} khỏi lịch sử`}
                onClick={() => void dropSearch(entry.query)}
              >
                <X size={12} aria-hidden="true" />
              </button>
            </span>
          ))}
          <button type="button" className="recent-chip__clear" onClick={() => void clearSearches()}>
            Xóa tất cả
          </button>
        </div>
      )}
      {historyError && (
        <p className="queue-error" role="alert">
          Không lưu được lịch sử tìm kiếm ({historyError}).
        </p>
      )}

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
