import {
  ChevronLeft,
  ChevronRight,
  Clock,
  Download,
  FolderOpen,
  Flame,
  Library,
  Link2,
  Search,
  Star,
} from "lucide-react";
import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import type { HomeFeedDto, HomeFeedItemDto } from "../../types/source";
import LocalContinueReading from "./components/LocalContinueReading";

/// The Home screen is fed by one vetted source's own front page. TruyenQQ is
/// a fixed product decision, not a user-selectable setting.
const HOME_SOURCE_ID = "truyenqq";
/// One full row per page in the "đang hot" rail — the mockup shows eight
/// covers across and pages instead of free-scrolling.
const HOT_PAGE_SIZE = 8;

function remotePath(item: HomeFeedItemDto) {
  return `/remote/${item.sourceId}/${item.sourceMangaId}`;
}

function CoverImage({ item, label }: { item: HomeFeedItemDto; label: string }) {
  const url = useCoverUrl(item.sourceId, item.sourceMangaId, item.coverKey);
  if (!url) {
    return (
      <span className="home-card__placeholder" aria-hidden="true">
        {item.title.trim().charAt(0).toUpperCase()}
      </span>
    );
  }
  return <img className="home-card__cover" src={url} alt={label} loading="lazy" />;
}

function Hero({ items }: { items: HomeFeedItemDto[] }) {
  const [index, setIndex] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const count = items.length;
  if (count === 0) {
    return null;
  }
  // Derived so a shrinking feed can never point past the end.
  const at = index % count;
  const active = items[at];
  const step = (delta: number) => setIndex((current) => (current + delta + count) % count);

  async function addToLibrary() {
    setSaving(true);
    setSaved(null);
    setSaveError(null);
    try {
      const result = await localServices.source.save({
        sourceId: active.sourceId,
        sourceMangaId: active.sourceMangaId,
      });
      setSaved(`Đã lưu ${result.savedChapterCount} chương`);
      bumpDomain("library");
      bumpDomain("home");
    } catch (error: unknown) {
      setSaveError(normalizeIpcError(error).message);
    } finally {
      setSaving(false);
    }
  }

  const meta = [active.latestChapter, active.latestTime].filter(Boolean).join(" · ");
  return (
    <section className="home-hero" aria-label="Nổi bật">
      <div className="home-hero__body">
        <div className="chip-row">
          <span className="chip chip--accent">
            <Flame size={12} aria-hidden="true" /> Nổi bật
          </span>
          <span className="chip">{active.sourceDisplayName}</span>
        </div>
        <h2 className="home-hero__title">{active.title}</h2>
        {meta && <p className="muted home-hero__meta">{meta}</p>}
        <div className="home-hero__actions">
          <Button variant="primary" to={remotePath(active)}>
            <Library size={16} aria-hidden="true" />
            Xem chi tiết
          </Button>
          <Button variant="secondary" onClick={() => void addToLibrary()} disabled={saving}>
            {saving ? "Đang lưu…" : "Thêm vào thư viện"}
          </Button>
        </div>
        {saved && (
          <p className="home-hero__note" role="status">
            {saved}
          </p>
        )}
        {saveError && (
          <p className="queue-error" role="alert">
            {saveError}
          </p>
        )}
      </div>
      <div className="home-hero__art">
        <CoverImage item={active} label={active.title} />
      </div>
      {count > 1 && (
        <>
          <button
            type="button"
            className="home-hero__arrow home-hero__arrow--left"
            onClick={() => step(-1)}
            aria-label="Truyện trước"
          >
            <ChevronLeft size={18} aria-hidden="true" />
          </button>
          <button
            type="button"
            className="home-hero__arrow home-hero__arrow--right"
            onClick={() => step(1)}
            aria-label="Truyện kế tiếp"
          >
            <ChevronRight size={18} aria-hidden="true" />
          </button>
          <ul className="home-hero__dots" aria-label="Chọn truyện nổi bật">
            {items.map((item, dot) => (
              <li key={item.sourceMangaId}>
                <button
                  type="button"
                  className={`home-hero__dot${dot === at ? " home-hero__dot--on" : ""}`}
                  onClick={() => setIndex(dot)}
                  aria-label={item.title}
                  aria-current={dot === at}
                />
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

function HotRail({ items }: { items: HomeFeedItemDto[] }) {
  const [page, setPage] = useState(0);
  const pages = Math.max(1, Math.ceil(items.length / HOT_PAGE_SIZE));
  const current = Math.min(page, pages - 1);
  const visible = items.slice(current * HOT_PAGE_SIZE, current * HOT_PAGE_SIZE + HOT_PAGE_SIZE);
  return (
    <section aria-label="Truyện đang hot">
      <div className="section-header">
        <h2>
          <Flame size={16} aria-hidden="true" /> Truyện đang hot
        </h2>
        <div className="home-pager">
          <Button to="/discover" variant="ghost">
            Xem tất cả
          </Button>
          <button
            type="button"
            className="home-pager__btn"
            onClick={() => setPage(current - 1)}
            disabled={current === 0}
            aria-label="Trang trước"
          >
            <ChevronLeft size={16} aria-hidden="true" />
          </button>
          <span className="muted home-pager__count">
            {current + 1}/{pages}
          </span>
          <button
            type="button"
            className="home-pager__btn"
            onClick={() => setPage(current + 1)}
            disabled={current >= pages - 1}
            aria-label="Trang kế tiếp"
          >
            <ChevronRight size={16} aria-hidden="true" />
          </button>
        </div>
      </div>
      <ul className="home-hot" aria-label="Danh sách truyện đang hot">
        {visible.map((item) => (
          <li key={item.sourceMangaId} className="home-card">
            <a className="home-card__link" href={`#${remotePath(item)}`}>
              <span className="home-card__art">
                <CoverImage item={item} label={item.title} />
              </span>
              <span className="home-card__title">{item.title}</span>
              {item.latestChapter && (
                <span className="muted home-card__chapter">{item.latestChapter}</span>
              )}
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}

function UpdatedRow({ item }: { item: HomeFeedItemDto }) {
  return (
    <li className="home-updated">
      <a className="home-updated__link" href={`#${remotePath(item)}`}>
        <span className="home-updated__art">
          <CoverImage item={item} label={item.title} />
        </span>
        <span className="home-updated__text">
          <span className="home-card__title">{item.title}</span>
          {item.latestChapter && (
            <span className="muted home-updated__chapter">{item.latestChapter}</span>
          )}
          {item.latestTime && <span className="home-updated__time">{item.latestTime}</span>}
        </span>
      </a>
    </li>
  );
}

const QUICK_ACTIONS = [
  {
    to: "/discover",
    label: "Tìm kiếm truyện",
    hint: "Khám phá toàn bộ thư viện nguồn",
    Icon: Search,
  },
  {
    to: "/discover?tab=url",
    label: "Thêm bằng URL",
    hint: "Nhập link từ các trang được hỗ trợ",
    Icon: Link2,
  },
  {
    to: "/downloads",
    label: "Xem tiến trình tải",
    hint: "Hàng đợi và tác vụ đang chạy",
    Icon: Download,
  },
  {
    to: "/library",
    label: "Mở thư viện",
    hint: "Quản lý truyện đã tải",
    Icon: FolderOpen,
  },
];

function QuickActions() {
  return (
    <nav className="panel home-quick" aria-label="Thao tác nhanh">
      <h2 className="panel__title">Thao tác nhanh</h2>
      <ul className="home-quick__list">
        {QUICK_ACTIONS.map(({ to, label, hint, Icon }) => (
          <li key={to}>
            <a className="home-quick__item" href={`#${to}`}>
              <span className="home-quick__icon">
                <Icon size={16} aria-hidden="true" />
              </span>
              <span className="home-quick__text">
                <span className="home-quick__label">{label}</span>
                <span className="muted home-quick__hint">{hint}</span>
              </span>
              <ChevronRight size={16} aria-hidden="true" className="home-quick__go" />
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}

export default function HomePage() {
  const homeVersion = useDomainVersion("home");
  const [retryNonce, setRetryNonce] = useState(0);
  const feed = useAsyncData<HomeFeedDto>(
    () => localServices.source.homeFeed(HOME_SOURCE_ID),
    [homeVersion, retryNonce],
  );

  if (feed.state === "loading") {
    return (
      <div className="page-stack" aria-busy="true" aria-label="Đang tải trang chủ">
        <Skeleton height="19rem" label="Đang tải truyện nổi bật" />
        <Skeleton height="15rem" label="Đang tải truyện đang hot" />
        <Skeleton height="14rem" label="Đang tải mới cập nhật" />
      </div>
    );
  }
  if (feed.state === "error") {
    const error = normalizeIpcError(feed.error);
    return (
      <ErrorState
        title="Không lấy được bảng tin trang chủ"
        description={`${error.message} (mã: ${error.code})`}
        action={
          <Button
            variant="primary"
            onClick={() => {
              bumpDomain("home");
              setRetryNonce((value) => value + 1);
            }}
          >
            Thử lại
          </Button>
        }
      />
    );
  }

  const { featured, newest, popular } = feed.data;
  return (
    <div className="home-layout">
      <div className="home-main page-stack">
        <Hero items={featured} />
        <HotRail items={featured} />
        <div className="home-split">
          <section aria-label="Cập nhật gần đây">
            <div className="section-header">
              <h2>
                <Clock size={16} aria-hidden="true" /> Cập nhật gần đây
              </h2>
              <Button to="/discover" variant="ghost">
                Xem tất cả
              </Button>
            </div>
            <ul className="home-updated-grid" aria-label="Truyện mới cập nhật">
              {newest.map((item) => (
                <UpdatedRow key={item.sourceMangaId} item={item} />
              ))}
            </ul>
          </section>
          <LocalContinueReading />
        </div>
      </div>

      <aside className="home-aside">
        <QuickActions />
        <section className="panel home-ranking" aria-label="Xem nhiều">
          <h2 className="panel__title">
            <Star size={16} aria-hidden="true" /> Xem nhiều
          </h2>
          <ol className="home-ranking__list">
            {popular.map((item) => (
              <li key={item.sourceMangaId}>
                <a href={`#${remotePath(item)}`}>{item.title}</a>
              </li>
            ))}
          </ol>
        </section>
      </aside>
    </div>
  );
}
