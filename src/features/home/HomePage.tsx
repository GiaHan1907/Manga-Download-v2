import { ChevronLeft, ChevronRight, Clock, Crown, Flame, Library } from "lucide-react";
import { useState } from "react";
import Button from "../../components/ui/Button";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { bumpDomain, useDomainVersion } from "../../shared/hooks/useInvalidation";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import type { HomeFeedDto, HomeFeedItemDto, RankingPeriod } from "../../types/source";
import LocalContinueReading from "./components/LocalContinueReading";

/// The Home screen is fed by one vetted source's own front page. TruyenQQ is
/// a fixed product decision, not a user-selectable setting.
const HOME_SOURCE_ID = "truyenqq";
/// Covers per page in the "đang hot" rail. Six keeps the row narrow enough
/// that the quick-actions rail still fits beside it at the default window width.
const HOT_PAGE_SIZE = 6;

const RANKING_TABS: { id: RankingPeriod; label: string }[] = [
  { id: "day", label: "Ngày" },
  { id: "month", label: "Tháng" },
  { id: "week", label: "Tuần" },
];

function Ranking({ seed }: { seed: HomeFeedItemDto[] }) {
  const [period, setPeriod] = useState<RankingPeriod>("day");
  const [rows, setRows] = useState<HomeFeedItemDto[]>(seed);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pick(next: RankingPeriod) {
    setPeriod(next);
    setBusy(true);
    setError(null);
    try {
      setRows(await localServices.source.ranking({ sourceId: HOME_SOURCE_ID, period: next }));
    } catch (caught: unknown) {
      setError(normalizeIpcError(caught).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel home-ranking" aria-label="Bảng xếp hạng">
      <div className="section-header">
        <h2 className="panel__title">
          <Crown size={16} aria-hidden="true" /> Bảng xếp hạng
        </h2>
        <Button to="/ranking" variant="ghost">
          Xem tất cả
        </Button>
      </div>
      <div className="home-tabs" role="tablist" aria-label="Kỳ xếp hạng">
        {RANKING_TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={period === tab.id}
            className={`home-tabs__tab${period === tab.id ? " home-tabs__tab--on" : ""}`}
            onClick={() => void pick(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {error && (
        <p className="queue-error" role="alert">
          {error}
        </p>
      )}
      <ol className="home-ranking__list" aria-busy={busy}>
        {rows.map((item, rank) => (
          <li key={`${period}-${item.sourceMangaId}`} className="home-ranking__row">
            <span
              className={`home-ranking__pos${rank < 3 ? " home-ranking__pos--medal" : ""}`}
            >
              {rank < 3 ? <Crown size={14} aria-hidden="true" /> : rank + 1}
            </span>
            {item.coverKey && (
              <span className="home-updated__art">
                <CoverImage item={item} label={item.title} />
              </span>
            )}
            <a className="home-ranking__text" href={`#${remotePath(item)}`}>
              <span className="home-card__title">{item.title}</span>
              {item.latestChapter && (
                <span className="muted home-updated__chapter">{item.latestChapter}</span>
              )}
            </a>
          </li>
        ))}
      </ol>
    </section>
  );
}

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
          <Button to="/ranking" variant="ghost">
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
        <section className="home-updated-block" aria-label="Cập nhật gần đây">
            <div className="section-header">
              <h2>
                <Clock size={16} aria-hidden="true" /> Cập nhật gần đây
              </h2>
              <Button to="/discover?status=new" variant="ghost">
                Xem tất cả
              </Button>
            </div>
            <ul className="home-updated-grid" aria-label="Truyện mới cập nhật">
              {newest.map((item) => (
                <UpdatedRow key={item.sourceMangaId} item={item} />
              ))}
            </ul>
          </section>
      </div>

      <aside className="home-aside">
        <Ranking seed={popular} />
        <LocalContinueReading />
      </aside>
    </div>
  );
}
