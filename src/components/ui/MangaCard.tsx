import { BookOpen } from "lucide-react";
import { Link } from "react-router-dom";
import StatusBadge from "./StatusBadge";

export type MangaCardProps = {
  title: string;
  subtitle: string;
  status?: string;
  coverUrl?: string;
  to?: string;
};

export default function MangaCard({ title, subtitle, status = "Mẫu giao diện", coverUrl, to }: MangaCardProps) {
  const inner = (
    <>
      <div className="manga-card__cover">
        {coverUrl ? <img src={coverUrl} alt="" /> : <BookOpen size={32} aria-hidden="true" />}
      </div>
      <div className="manga-card__body">
        <h3 className="manga-card__title">{title}</h3>
        <p className="manga-card__meta">{subtitle}</p>
        <div style={{ marginTop: "var(--space-3)" }}>
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
