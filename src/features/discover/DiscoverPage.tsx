import { Compass, Link2, Sparkles } from "lucide-react";
import { Link, useSearchParams } from "react-router-dom";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import Tabs from "../../components/ui/Tabs";
import { services } from "../../services/provider";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useServicesVersion } from "../../shared/hooks/useServices";
import DiscoverSourcesPanel from "./DiscoverSourcesPanel";
import DiscoverSuggestions from "./DiscoverSuggestions";
import LiveSearchPanel from "./LiveSearchPanel";
import LiveUrlImportPanel from "./LiveUrlImportPanel";
import UrlImportPanel from "./components/UrlImportPanel";
import SearchPanel from "./components/SearchPanel";

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
  const showDemo =
    import.meta.env.DEV && (requested === "demo" || requested === "demo-url");

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

  return (
    <div className="page-stack">
      <section className="discover-hero" aria-label="Giới thiệu Khám phá">
        <div className="discover-hero__text">
          <h1 className="discover-hero__title">
            Khám phá <span className="discover-hero__accent">truyện</span>
          </h1>
          <p className="muted">
            Tìm truyện theo tên hoặc tác giả, nhập URL từ nguồn được hỗ trợ, hoặc lưu nhanh một bộ đang nổi bật.
          </p>
        </div>
        <span className="discover-hero__mark" aria-hidden="true">
          <Compass size={26} />
        </span>
      </section>

      <div className="discover-layout">
        <div className="discover-main page-stack">
          <section aria-label="Tìm kiếm truyện">
            <div className="section-header">
              <h2>
                <Sparkles size={16} aria-hidden="true" /> Tìm kiếm truyện
              </h2>
            </div>
            <LiveSearchPanel initialQuery={onlineQuery} />
          </section>

          <section className="panel" aria-label="Thêm bằng URL">
            <div className="section-header">
              <h2 className="panel__title">
                <Link2 size={16} aria-hidden="true" /> Thêm bằng URL
              </h2>
            </div>
            <p className="muted discover-url__hint">
              Dán liên kết một truyện thuộc nguồn đã đăng ký để phân tích và thêm vào thư viện.
            </p>
            <LiveUrlImportPanel />
          </section>

          <section aria-label="Truyện nổi bật trên nguồn">
            <div className="section-header">
              <h2>
                <Sparkles size={16} aria-hidden="true" /> Đề xuất cho bạn
              </h2>
            </div>
            <p className="muted discover-suggestions__caption">
              Những bộ đang nổi bật trên TruyenQQ — chưa có gợi ý theo sở thích cá nhân.
            </p>
            <DiscoverSuggestions />
          </section>
        </div>

        <aside className="discover-aside">
          <DiscoverSourcesPanel />
        </aside>
      </div>
    </div>
  );
}
