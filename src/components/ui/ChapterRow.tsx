import StatusBadge from "./StatusBadge";

export type ChapterRowProps = {
  title: string;
  meta: string;
  status?: string;
};

export default function ChapterRow({ title, meta, status = "Phase 03" }: ChapterRowProps) {
  return (
    <article className="chapter-row">
      <div>
        <h3 className="chapter-row__title">{title}</h3>
        <p className="chapter-row__meta">{meta}</p>
      </div>
      <StatusBadge tone="neutral">{status}</StatusBadge>
    </article>
  );
}
