import { ChevronLeft, ChevronRight, Compass, Star, Flame, Clock } from "lucide-react";
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

function MangaTile({ item }: { item: HomeFeedItemDto }) {
  return (
    <li className="home-card">
      <a className="home-card__link" href={`#/remote/${item.sourceId}/${item.sourceMangaId}`}>
        <span className="home-card__art">
          <CoverImage item={item} label={item.title} />
        </span>
        <span className="home-card__title">{item.title}</span>
        {item.latestChapter && <span className="muted home-card__chapter">{item.latestChapter}</span>}
      </a>
    </li>
  );
}

function Hero({ items }: { items: HomeFeedItemDto[] }) {
  const [index, setIndex] = useState(0);
  const count = items.length;
  if (count === 0) {
    return null;
  }
  // Derived so a shrinking feed can never point past the end.
  const at = index % count;
  const active = items[at];
  const step = (delta: number) => setIndex((current) => (current + delta + count) % count);
  return (
    <section className="home-hero-card" aria-label="Nổi bật">
      <div className="home-hero-card__art">
        <CoverImage item={active} label={active.title} />
      </div>
      <div className="home-hero-card__body">
        <span className="chip chip--accent">
          <Flame size={12} aria-hidden="true" /> Nổi bật
        </span>
        <h2 className="home-hero-card__title">{active.title}</h2>
        {active.latestChapter && (
          <p className="muted home-hero-card__chapter">{active.latestChapter}</p>
        )}
        <div className="home-hero-card__actions">
          <Button variant="primary" to={`/remote/${active.sourceId}/${active.sourceMangaId}`}>
            Xem chi tiết
          </Button>
          <Button variant="ghost" to="/discover">
            <Compass size={16} aria-hidden="true" />
            Khám phá
          </Button>
        </div>
      </div>
      {count > 1 && (
        <div className="home-hero-card__nav">
          <button type="button" className="home-hero-card__arrow" onClick={() => step(-1)} aria-label="Truyện trước">
            <ChevronLeft size={18} aria-hidden="true" />
          </button>
          <ul className="home-hero-card__dots" aria-label="Chọn truyện nổi bật">
            {items.map((item, dot) => (
              <li key={item.sourceMangaId}>
                <button
                  type="button"
                  className={`home-hero-card__dot${dot === at ? " home-hero-card__dot--on" : ""}`}
                  onClick={() => setIndex(dot)}
                  aria-label={item.title}
                  aria-current={dot === at}
                />
              </li>
            ))}
          </ul>
          <button type="button" className="home-hero-card__arrow" onClick={() => step(1)} aria-label="Truyện kế tiếp">
            <ChevronRight size={18} aria-hidden="true" />
          </button>
        </div>
      )}
    </section>
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
        <Skeleton height="18rem" label="Đang tải truyện nổi bật" />
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
    <div className="page-stack">
      <Hero items={featured} />

      <section aria-label="Mới cập nhật">
        <div className="section-header">
          <h2>
            <Clock size={16} aria-hidden="true" /> Mới cập nhật
          </h2>
          <Button to="/discover" variant="ghost">
            Xem tất cả
          </Button>
        </div>
        <ul className="home-grid" aria-label="Truyện mới cập nhật">
          {newest.map((item) => (
            <MangaTile key={item.sourceMangaId} item={item} />
          ))}
        </ul>
      </section>

      <div className="home-split">
        <LocalContinueReading />
        <section className="panel home-ranking" aria-label="Xem nhiều">
          <h2 className="panel__title">
            <Star size={16} aria-hidden="true" /> Xem nhiều
          </h2>
          <ol className="home-ranking__list">
            {popular.map((item) => (
              <li key={item.sourceMangaId}>
                <a href={`#/remote/${item.sourceId}/${item.sourceMangaId}`}>{item.title}</a>
              </li>
            ))}
          </ol>
        </section>
      </div>
    </div>
  );
}
