import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowUpDown, CircleDot, History, Search, SlidersHorizontal, Tags, X } from "lucide-react";
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
  BrowseSort,
  BrowseStatus,
  GenreTagDto,
  RemoteMangaSummaryDto,
  SearchResultDto,
  SourceViewDto,
} from "../../types/source";
import { BROWSE_SORT_LABEL, BROWSE_STATUS_LABEL } from "../../types/source";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";

/// Catalog source for genre/status browsing. Only adapters with a real
/// catalog page implement `browse`, and `genres()` returning an error is how
/// the others opt out — the pill then disappears rather than lying.
const CATALOG_SOURCE_ID = "truyenqq";

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

/// A deep link may start the screen in catalog mode, but only with a value the
/// source itself publishes — anything else falls back to keyword search.
export type CatalogSelection = {
  genre: string;
  status: BrowseStatus | "";
  sort: BrowseSort | "";
};

export default function LiveSearchPanel({
  initialQuery = "",
  initialCatalog,
}: {
  initialQuery?: string;
  initialCatalog?: CatalogSelection;
}) {
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
    if (value.length < 2 || browsing) {
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

  // Catalog browsing is a different mode from keyword search: the source's
  // genre/status pages carry no keyword, so the two cannot be combined here.
  // Sort is a modifier of a genre page, never a mode of its own — the source
  // ignores `?sort=` on its status routes, so picking it alone would lie.
  const [genre, setGenre] = useState(initialCatalog?.genre ?? "");
  const [status, setStatus] = useState<BrowseStatus | "">(initialCatalog?.status ?? "");
  // A sort needs a genre to act on, so a deep link cannot carry one without it.
  const [sort, setSort] = useState<BrowseSort | "">(
    initialCatalog && initialCatalog.genre ? initialCatalog.sort : "",
  );
  const browsing = genre !== "" || status !== "";
  const genres = useAsyncData<GenreTagDto[]>(
    () => localServices.source.genres(CATALOG_SOURCE_ID),
    [sourcesVersion],
  );
  // A source with no published genre vocabulary gets no genre pill.
  const genreOptions = genres.state === "success" ? genres.data : [];

  // One search identity: query + source filter + manual reload. Page 1 is
  // always refetched on change; extra pages are dropped render-side (the
  // React derived-state pattern — no effect, no stale-frame flash).
  const searchKey = [
    browsing ? "browse" : "q",
    browsing ? `${genre}|${status}|${sort}` : term,
    effectiveIds?.join(",") ?? "all",
    "r",
    reload,
  ].join("|");
  const [more, setMore] = useState<MoreState>(EMPTY_MORE);
  if (more.key !== searchKey) {
    setMore({ key: searchKey, pages: [], loading: false, error: null });
  }

  const results = useAsyncData<SearchResultDto | null>(
    () => {
      if (browsing) {
        return localServices.source.browse({
          sourceId: CATALOG_SOURCE_ID,
          genre: genre || undefined,
          status: status || undefined,
          sort: sort || undefined,
          page: 1,
        });
      }
      return term.length < 2
        ? Promise.resolve(null)
        : localServices.source.search({ query: term, sourceIds: effectiveIds, page: 1 });
    },
    // effectiveIds key: disabling/removing a source must invalidate rendered
    // results that were gathered from sources which are no longer searchable.
    [
      term,
      sourceFilter,
      effectiveIds?.join(",") ?? "all",
      reload,
      browsing,
      genre,
      status,
      sort,
    ],
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
    if (more.loading || (!browsing && term.length < 2)) {
      return;
    }
    const key = searchKey;
    const nextPage = more.pages.length + 2;
    setMore((current) => (current.key === key ? { ...current, loading: true, error: null } : current));
    try {
      const page = browsing
        ? await localServices.source.browse({
            sourceId: CATALOG_SOURCE_ID,
            genre: genre || undefined,
            status: status || undefined,
            sort: sort || undefined,
            page: nextPage,
          })
        : await localServices.source.search({
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
          disabled={browsing}
          placeholder="Nhập tên truyện, tác giả, hoặc từ khóa (tối thiểu 2 ký tự)..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <Button variant="primary" type="submit" disabled={browsing || query.trim().length < 2}>
          Tìm kiếm
        </Button>
      </form>

      <div className="discover-filters">
        <label className="filter-pill">
          <SlidersHorizontal size={13} aria-hidden="true" />
          <span>Nguồn:</span>
          <select
            value={sourceFilter}
            disabled={searchable.length === 0 || browsing}
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
        {genreOptions.length > 0 && (
          <label className="filter-pill">
            <Tags size={13} aria-hidden="true" />
            <span>Thể loại:</span>
            <select
              value={genre}
              onChange={(event) => {
                setGenre(event.target.value);
                // Sorting lives on the genre page only, so dropping the genre
                // must drop the sort rather than leave it pending and ignored.
                if (event.target.value === "") setSort("");
              }}
            >
              <option value="">Tất cả</option>
              {genreOptions.map((tag) => (
                <option key={tag.slug} value={tag.slug}>
                  {tag.label}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className="filter-pill">
          <CircleDot size={13} aria-hidden="true" />
          <span>Trạng thái:</span>
          <select
            value={status}
            onChange={(event) => setStatus(event.target.value as BrowseStatus | "")}
          >
            <option value="">Tất cả</option>
            {(Object.keys(BROWSE_STATUS_LABEL) as BrowseStatus[]).map((value) => (
              <option key={value} value={value}>
                {BROWSE_STATUS_LABEL[value]}
              </option>
            ))}
          </select>
        </label>
        {genreOptions.length > 0 && (
          <label className="filter-pill">
            <ArrowUpDown size={13} aria-hidden="true" />
            <span>Sắp xếp:</span>
            <select
              value={sort}
              disabled={genre === ""}
              title={genre === "" ? "Nguồn chỉ sắp xếp được trong một thể loại" : undefined}
              onChange={(event) => setSort(event.target.value as BrowseSort | "")}
            >
              <option value="">Mặc định</option>
              {(Object.keys(BROWSE_SORT_LABEL) as BrowseSort[]).map((value) => (
                <option key={value} value={value}>
                  {BROWSE_SORT_LABEL[value]}
                </option>
              ))}
            </select>
          </label>
        )}
        {searchable.length === 0 && !browsing && (
          <p className="muted">Chưa có nguồn nào bật hỗ trợ tìm kiếm. Đăng ký trong Cài đặt → Nguồn.</p>
        )}
        {browsing && (
          <p className="muted discover-filters__note">
            Đang xem danh mục theo bộ lọc — nguồn không nhận từ khóa kèm bộ lọc nên ô tìm kiếm tạm
            ngưng, và chỉ sắp xếp được bên trong một thể loại.
          </p>
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
            title={browsing ? "Danh mục này đang trống" : "Không có kết quả"}
            description={
              browsing
                ? "Nguồn không có truyện nào ở bộ lọc đã chọn."
                : "Các nguồn đang bật đều trả về rỗng. Thử từ khóa khác hoặc bỏ bớt bộ lọc nguồn."
            }
            action={
              browsing ? (
                <Button
                  onClick={() => {
                    setGenre("");
                    setStatus("");
                    setSort("");
                  }}
                >
                  Bỏ bộ lọc
                </Button>
              ) : (
                <Button onClick={() => setQuery("")}>Xóa từ khóa</Button>
              )
            }
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
