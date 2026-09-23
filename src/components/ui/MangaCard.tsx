import { BookOpen } from "lucide-react";
import { Link } from "react-router-dom";
import StatusBadge from "./StatusBadge";

export type MangaCardProps = {
  title: string;
  subtitle: string;
  status?: string;
  coverUrl?: string;
  to?: string;
  /** Feature plan F1: unacknowledged new-chapter alerts → red badge. */
  alertCount?: number;
};

export default function MangaCard({ title, subtitle, status = "Mẫu giao diện", coverUrl, to, alertCount }: MangaCardProps) {
  const inner = (
    <>
      <div className="manga-card__cover">
        {coverUrl ? <img src={coverUrl} alt="" /> : <BookOpen size={32} aria-hidden="true" />}
      </div>
      <div className="manga-card__body">
        <h3 className="manga-card__title">{title}</h3>
        <p className="manga-card__meta">{subtitle}</p>
        <div style={{ marginTop: "var(--space-3)" }}>
          {(alertCount ?? 0) > 0 ? (
            <span className="manga-card__alert" role="status">
              {alertCount} chương mới
            </span>
          ) : null}
          <StatusBadge>{status}</StatusBadge>
        </div>
      </div>
    </>
  );
  if (to) {
    return (
      <Link to={to} className="manga-card manga-card--link">
        {inner}
      </Link>
    );
  }
  return <article className="manga-card">{inner}</article>;
}
