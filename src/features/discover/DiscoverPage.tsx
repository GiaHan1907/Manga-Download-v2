import { Search } from "lucide-react";
import { useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import Button from "../../components/ui/Button";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Input from "../../components/ui/Input";
import MangaCard from "../../components/ui/MangaCard";
import Skeleton from "../../components/ui/Skeleton";
import Tabs from "../../components/ui/Tabs";
import type { MangaSummary } from "../../domain/manga";
import { services } from "../../services/provider";
import type { CatalogSource } from "../../services/catalog.service";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";
import UrlImportPanel from "./components/UrlImportPanel";
import { useDebouncedValue } from "./hooks/useDebouncedValue";

function SearchPanel({ sources }: { sources: CatalogSource[] }) {
  const version = useServicesVersion();
  const [query, setQuery] = useState("");
  const [selectedSources, setSelectedSources] = useState<string[]>([]);
  const debouncedQuery = useDebouncedValue(query, 300);

  const enabledSourceIds = sources.filter((source) => source.enabled).map((source) => source.id);
  const effectiveSourceIds = (
    selectedSources.length > 0 ? selectedSources : enabledSourceIds
  ).filter((id) => enabledSourceIds.includes(id));

  const results = useAsyncData<MangaSummary[]>(
    () =>
      effectiveSourceIds.length === 0
        ? Promise.resolve([])
        : services.catalog.search(debouncedQuery, effectiveSourceIds),
    [debouncedQuery, selectedSources, version],
  );

  const enabledSources = sources.filter((source) => source.enabled);

  function toggleSource(sourceId: string) {
    setSelectedSources((current) =>
      current.includes(sourceId) ? current.filter((id) => id !== sourceId) : [...current, sourceId],
    );
  }

  function clearFilters() {
    setQuery("");
    setSelectedSources([]);
  }

  return (
    <div className="page-stack">
      <div className="discover-controls">
        <Input
          label="Tìm kiếm demo"
          type="search"
          placeholder="Nhập tên, tác giả hoặc thể loại (bỏ dấu vẫn khớp)..."
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          hint="Debounce 300ms, chỉ khớp fixture nội bộ — không có tìm kiếm thật."
        />
        <fieldset className="source-filter" aria-label="Lọc theo nguồn demo">
          <legend>Nguồn</legend>
          {enabledSources.length === 0 && <p className="muted">Không có nguồn nào đang bật. Bật lại trong Settings → Sources.</p>}
          {enabledSources.map((source) => (
            <label key={source.id} className="source-filter__option">
              <input type="checkbox" checked={selectedSources.includes(source.id)} onChange={() => toggleSource(source.id)} />
              {source.label}
            </label>
          ))}
        </fieldset>
      </div>

      {results.state === "loading" && (
        <div className="card-grid" aria-busy="true">
          <Skeleton height="18rem" />
          <Skeleton height="18rem" />
          <Skeleton height="18rem" />
        </div>
      )}
      {results.state === "error" && (
        <ErrorState title="Dịch vụ demo lỗi" description="Không thực hiện được truy vấn mẫu. Thử lại hoặc đổi bộ lọc." />
      )}
      {results.state === "success" && results.data.length === 0 && (
        <EmptyState
          title="Không có kết quả mẫu"
          description="Câu truy vấn hoặc bộ lọc nguồn không khớp fixture nào. Thử từ khóa khác như “hanh trinh” hoặc “giấy”."
          action={<Button onClick={clearFilters}>Xóa bộ lọc</Button>}
        />
      )}
      {results.state === "success" && (
        <div className="card-grid">
          {results.data.map((manga) => (
            <Link key={manga.id} to={`/manga/${manga.id}`} className="card-link" aria-label={`Mở ${manga.title}`}>
              <MangaCard
                title={manga.title}
                subtitle={`${manga.author ?? "Không rõ tác giả"} · ${manga.genres.join(", ")}`}
                status={manga.status}
                coverUrl={manga.coverAsset}
              />
            </Link>
          ))}
        </div>
      )}
      <p className="muted">
        <Search size={13} aria-hidden="true" /> Kết quả hiển thị theo thứ tự fixture ổn định; chọn một thẻ để mở Manga Detail.
      </p>
    </div>
  );
}

export default function DiscoverPage() {
  const version = useServicesVersion();
  const [searchParams] = useSearchParams();
  const sources = useAsyncData(() => services.catalog.listSources(), [version]);
  const initialTab = searchParams.get("tab") === "url" ? "url" : "search";

  const tabs = useMemo(() => {
    if (sources.state !== "success") {
      return null;
    }
    return [
      {
        id: "search",
        label: "Tìm kiếm demo",
        content: <SearchPanel sources={sources.data} />,
      },
      {
        id: "url",
        label: "Import URL (demo)",
        content: <UrlImportPanel catalog={services.catalog} />,
      },
    ];
  }, [sources]);

  if (sources.state === "loading") {
    return <Skeleton height="12rem" label="Đang tải nguồn demo" />;
  }
  if (sources.state === "error" || !tabs) {
    return <ErrorState title="Không tải được danh sách nguồn demo" description="Dịch vụ catalog lỗi. Reload để thử lại." />;
  }

  return (
    <div className="page-stack">
      <Tabs items={tabs} defaultValue={initialTab} />
    </div>
  );
}
