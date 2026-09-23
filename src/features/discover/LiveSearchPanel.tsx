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
import type { RemoteMangaSummaryDto, SearchResultDto, SourceViewDto } from "../../types/source";
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

  const results = useAsyncData<SearchResultDto | null>(
    () =>
      debounced.length < 2
        ? Promise.resolve(null)
        : localServices.source.search({ query: debounced, sourceIds: effectiveIds }),
    // effectiveIds key: disabling/removing a source must invalidate rendered
    // results that were gathered from sources which are no longer searchable.
    [debounced, selected.join(","), effectiveIds?.join(",") ?? "all", reload],
  );

  function toggleSource(sourceId: string) {
    setSelected((current) =>
      current.includes(sourceId) ? current.filter((id) => id !== sourceId) : [...current, sourceId],
    );
  }

  if (sources.state === "loading") {
    return <Skeleton height="10rem" label="Đang tải danh sách nguồn thật" />;
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

  const failedSources = results.state === "success" ? results.data?.sourceErrors ?? [] : [];

  return (
    <div className="page-stack">
      <div className="discover-controls">
        <Input
          label="Tìm kiếm thật (qua Rust source engine)"
          type="search"
          placeholder="Nhập tên truyện (tối thiểu 2 ký tự)..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          hint="Debounce 300ms; truy vấn được gửi tới từng adapter đã đăng ký, lỗi từng nguồn hiển thị riêng."
        />
        <fieldset className="source-filter" aria-label="Lọc theo nguồn thật">
          <legend>Nguồn</legend>
          {searchable.length === 0 && (
            <p className="muted">Chưa có nguồn thật nào bật hỗ trợ tìm kiếm. Đăng ký trong Settings → Sources.</p>
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

      {results.state === "loading" && (
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
          description="Nhập ít nhất 2 ký tự để tìm qua các nguồn thật đã bật."
        />
      )}
      {results.state === "success" &&
        results.data !== null &&
        results.data.items.length === 0 &&
        failedSources.length === 0 && (
          <EmptyState
            title="Không có kết quả"
            description="Các nguồn đang bật đều trả về rỗng. Thử từ khóa khác hoặc bỏ bớt bộ lọc nguồn."
            action={<Button onClick={() => setQuery("")}>Xóa từ khóa</Button>}
          />
        )}
      {results.state === "success" && results.data !== null && results.data.items.length > 0 && (
        <div className="card-grid">
          {results.data.items.map((item) => (
            <LiveSearchCard
              key={`${item.sourceId}:${item.sourceMangaId}`}
              item={item}
            />
          ))}
        </div>
      )}
      <p className="muted">
        Kết quả là metadata từ xa — chưa có gì được lưu hay tải về. Mở một mục để xem trước chi tiết.
      </p>
    </div>
  );
}
