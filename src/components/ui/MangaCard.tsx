import { BookOpen } from "lucide-react";
import StatusBadge from "./StatusBadge";

export type MangaCardProps = {
  title: string;
  subtitle: string;
  status?: string;
  coverUrl?: string;
};

export default function MangaCard({ title, subtitle, status = "Mẫu giao diện", coverUrl }: MangaCardProps) {
  return (
    <article className="manga-card">
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
    </article>
  );
}
