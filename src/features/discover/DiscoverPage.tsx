import { Compass, Flame, Link2 } from "lucide-react";
import { Link, useSearchParams } from "react-router-dom";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import Tabs from "../../components/ui/Tabs";
import { localServices, services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import { useDomainVersion } from "../../shared/hooks/useInvalidation";
import { useServicesVersion } from "../../shared/hooks/useServices";
import type { HomeFeedDto, HomeFeedItemDto } from "../../types/source";
import DiscoverSourcesPanel from "./DiscoverSourcesPanel";
import DiscoverSuggestions from "./DiscoverSuggestions";
import LiveSearchPanel from "./LiveSearchPanel";
import LiveUrlImportPanel from "./LiveUrlImportPanel";
import UrlImportPanel from "./components/UrlImportPanel";
import SearchPanel from "./components/SearchPanel";

/// The screen's spotlight source — the same vetted rail Home reads.
const SOURCE_ID = "truyenqq";

/// The mockup's hero carries artwork. A real cover from the spotlight rail is
/// the only honest image available, so the first featured story stands in.
function HeroArt({ item }: { item: HomeFeedItemDto | undefined }) {
  const cover = useCoverUrl(item?.sourceId ?? "", item?.sourceMangaId ?? "", item?.coverKey ?? null);
  if (!item) {
    return (
      <span className="discover-hero__mark" aria-hidden="true">
        <Compass size={26} />
      </span>
    );
  }
  return (
    <span className="discover-hero__art" aria-hidden="true">
      {cover ? <img src={cover} alt="" /> : <Compass size={26} />}
    </span>
  );
}

/// Dev-only fixture sandbox, kept reachable on its old routes so existing
/// tests and bookmarks keep working while the live screen is the default.
function DemoSandbox({ initialTab }: { initialTab: string }) {
  const version = useServicesVersion();
  const sources = useAsyncData(() => services.catalog.listSources(), [version]);

  if (sources.state === "loading") {
    return <Skeleton height="12rem" label="Đang tải nguồn demo" />;
  }
  if (sources.state === "error") {
    return (
      <ErrorState
        title="Không tải được danh sách nguồn demo"
        description="Dịch vụ catalog lỗi. Reload để thử lại."
      />
    );
  }
  return (
    <Tabs
      defaultValue={initialTab === "url" ? "url" : "search"}
      items={[
        { id: "search", label: "Tìm kiếm demo", content: <SearchPanel sources={sources.data} /> },
        { id: "url", label: "Import URL (demo)", content: <UrlImportPanel catalog={services.catalog} /> },
      ]}
    />
  );
}

export default function DiscoverPage() {
  const [searchParams] = useSearchParams();
  const requested = searchParams.get("tab") ?? "search";
  const onlineQuery = searchParams.get("q") ?? "";
  const homeVersion = useDomainVersion("home");
  // Fetched once for the whole screen: the hero art and the suggestion rail
  // read the same payload, and the source is paced per session.
  const feed = useAsyncData<HomeFeedDto>(
    () => localServices.source.homeFeed(SOURCE_ID),
    [homeVersion],
  );

  const showDemo = import.meta.env.DEV && (requested === "demo" || requested === "demo-url");

  // Rendered as its own screen rather than a section of the live page: two
  // import boxes on one screen made the same control name ambiguous.
  if (showDemo) {
    return (
      <div className="page-stack">
        <div className="section-header">
          <h1>Demo sandbox (chỉ bản dev)</h1>
          <Link to="/discover" className="muted">
            Về Khám phá thật
          </Link>
        </div>
        <DemoSandbox initialTab={requested === "demo-url" ? "url" : "search"} />
      </div>
    );
  }

  const spotlight = feed.state === "success" ? feed.data.featured[0] : undefined;

  return (
    <div className="page-stack">
      <section className="discover-hero" aria-label="Giới thiệu Khám phá">
        <div className="discover-hero__top">
          <div className="discover-hero__text">
            <h1 className="discover-hero__title">
              Khám phá <span className="discover-hero__accent">truyện</span>
            </h1>
            <p className="muted">Tìm truyện theo tên, tác giả hoặc nhập URL từ nguồn được hỗ trợ.</p>
          </div>
          <HeroArt item={spotlight} />
        </div>
        <LiveSearchPanel initialQuery={onlineQuery} />
      </section>

      <div className="discover-layout">
        <div className="discover-main page-stack">
          <section className="panel" aria-label="Thêm bằng URL">
            <div className="section-header">
              <h2 className="panel__title">
                <Link2 size={16} aria-hidden="true" /> Thêm bằng URL
              </h2>
            </div>
            <p className="muted discover-url__hint">
              Dán link từ các trang truyện được hỗ trợ để phân tích và thêm vào thư viện.
            </p>
            <LiveUrlImportPanel />
          </section>

          <section aria-label="Truyện nổi bật trên nguồn">
            <div className="section-header discover-suggestions__head">
              <h2>
                <Flame size={16} aria-hidden="true" /> Đề xuất cho bạn
              </h2>
              <p className="muted">Những bộ đang nổi bật trên TruyenQQ — chưa có gợi ý theo sở thích.</p>
            </div>
            <DiscoverSuggestions feed={feed} />
          </section>
        </div>

        <aside className="discover-aside">
          <DiscoverSourcesPanel />
        </aside>
      </div>
    </div>
  );
}
