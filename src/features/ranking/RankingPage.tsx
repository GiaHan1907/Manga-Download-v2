import { Crown } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import EmptyState from "../../components/ui/EmptyState";
import ErrorState from "../../components/ui/ErrorState";
import Skeleton from "../../components/ui/Skeleton";
import { localServices } from "../../services/provider";
import { normalizeIpcError } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { useDomainVersion } from "../../shared/hooks/useInvalidation";
import { useCoverUrl } from "../../shared/hooks/useCoverUrl";
import type { HomeFeedItemDto, RankingPeriod } from "../../types/source";

/// The same three rails Home shows in its side column, given a full screen.
/// Ranks are the source's own ordering — no view counts are displayed because
/// the ranking pages publish none.
const SOURCE_ID = "truyenqq";

const PERIODS: { id: RankingPeriod; label: string; word: string }[] = [
  { id: "day", label: "Theo ngày", word: "ngày" },
  { id: "week", label: "Theo tuần", word: "tuần" },
  { id: "month", label: "Theo tháng", word: "tháng" },
];

function Cover({ item }: { item: HomeFeedItemDto }) {
  const url = useCoverUrl(item.sourceId, item.sourceMangaId, item.coverKey);
  if (!url) {
    return (
      <span className="ranking-row__placeholder" aria-hidden="true">
        {item.title.trim().charAt(0).toUpperCase()}
      </span>
    );
  }
  return <img className="ranking-row__cover" src={url} alt="" loading="lazy" />;
}

function Row({ item, rank }: { item: HomeFeedItemDto; rank: number }) {
  return (
    <li className="ranking-row">
      <span className={`ranking-row__pos${rank < 3 ? " ranking-row__pos--medal" : ""}`}>
        {rank < 3 ? <Crown size={14} aria-hidden="true" /> : rank + 1}
      </span>
      <span className="ranking-row__art">
        <Cover item={item} />
      </span>
      <Link
        to={`/remote/${encodeURIComponent(item.sourceId)}/${encodeURIComponent(item.sourceMangaId)}`}
        className="ranking-row__text"
      >
        <span className="ranking-row__title">{item.title}</span>
        <span className="muted ranking-row__meta">
          {[item.latestChapter, item.latestTime].filter(Boolean).join(" · ") || item.sourceDisplayName}
        </span>
      </Link>
    </li>
  );
}

export default function RankingPage() {
  const [period, setPeriod] = useState<RankingPeriod>("day");
  const homeVersion = useDomainVersion("home");
  const rows = useAsyncData(
    () => localServices.source.ranking({ sourceId: SOURCE_ID, period }),
    [period, homeVersion],
  );

  return (
    <div className="page-stack">
      <section className="panel" aria-label="Bảng xếp hạng">
        <div className="section-header">
          <h2 className="panel__title">
            <Crown size={16} aria-hidden="true" /> Bảng xếp hạng
          </h2>
          <p className="muted panel__description">
            {`Thứ tự do chính TruyenQQ công bố theo ${PERIODS.find((p) => p.id === period)?.word}.`}
          </p>
        </div>

        <div className="home-tabs" role="tablist" aria-label="Kỳ xếp hạng">
          {PERIODS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={period === tab.id}
              className={`home-tabs__tab${period === tab.id ? " home-tabs__tab--on" : ""}`}
              onClick={() => setPeriod(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {rows.state === "loading" && (
          <div className="ranking-list" aria-busy="true">
            <Skeleton height="3.5rem" />
            <Skeleton height="3.5rem" />
            <Skeleton height="3.5rem" />
          </div>
        )}
        {rows.state === "error" && (
          <ErrorState
            title="Không tải được bảng xếp hạng"
            description={normalizeIpcError(rows.error).message}
          />
        )}
        {rows.state === "success" && rows.data.length === 0 && (
          <EmptyState
            title="Nguồn chưa trả về xếp hạng"
            description="Nguồn không công bố bảng xếp hạng cho kỳ này, hoặc đang chặn truy cập."
          />
        )}
        {rows.state === "success" && rows.data.length > 0 && (
          <ol className="ranking-list">
            {rows.data.map((item, rank) => (
              <Row key={`${period}-${item.sourceId}-${item.sourceMangaId}`} item={item} rank={rank} />
            ))}
          </ol>
        )}
      </section>
    </div>
  );
}
