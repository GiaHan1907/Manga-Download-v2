import { Link } from "react-router-dom";
import Skeleton from "../../components/ui/Skeleton";
import { listRecentActivity } from "../../shared/api/desktop";
import { useAsyncData } from "../../shared/hooks/useAsyncData";
import { relativeAge } from "../../shared/lib/time";

const KIND_LABEL = {
  alert: "Chương mới",
  downloaded: "Đã tải xong",
  failed: "Tải lỗi",
} as const;

/// The toast is gone in two seconds and the queue forgets its history, so
/// anything that happened while the user was elsewhere is only in the DB.
/// This reads it back: newest first, each row links to its manga.
export default function ActivityPanel() {
  const feed = useAsyncData(() => listRecentActivity({ limit: 12 }), []);

  return (
    <section className="panel" aria-label="Hoạt động gần đây">
      <h2 className="panel__title">Hoạt động gần đây</h2>
      <p className="panel__description">
        Chương mới được phát hiện, chương tải xong và tải lỗi — mới nhất trước.
      </p>
      {feed.state === "loading" && <Skeleton height="4rem" label="Đang đọc nhật ký" />}
      {feed.state === "error" && <p className="muted">Không đọc được nhật ký hoạt động.</p>}
      {feed.state === "success" && feed.data.length === 0 && (
        <p className="muted">Chưa có hoạt động nào.</p>
      )}
      {feed.state === "success" && feed.data.length > 0 && (
        <ul className="activity-feed">
          {feed.data.map((item) => (
            <li
              key={`${item.kind}-${item.mangaId}-${item.chapterTitle}-${item.at}`}
              className="activity-feed__row"
            >
              <span className={`activity-feed__kind activity-feed__kind--${item.kind}`}>
                {KIND_LABEL[item.kind]}
              </span>
              <Link className="activity-feed__title" to={`/library/${item.mangaId}`}>
                {item.mangaTitle}
              </Link>
              <span className="activity-feed__chapter">{item.chapterTitle}</span>
              {item.code && <span className="activity-feed__code">{item.code}</span>}
              <time className="activity-feed__age" dateTime={item.at}>
                {relativeAge(item.at)}
              </time>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
