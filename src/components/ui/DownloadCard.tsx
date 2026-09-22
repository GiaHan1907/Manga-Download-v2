import Progress from "./Progress";
import StatusBadge from "./StatusBadge";

export type DownloadCardProps = {
  title: string;
  description: string;
  progress: number;
  status?: string;
};

export default function DownloadCard({ title, description, progress, status = "Mẫu trạng thái" }: DownloadCardProps) {
  return (
    <article className="download-card">
      <div>
        <h3 className="download-card__title">{title}</h3>
        <p className="download-card__meta">{description}</p>
      </div>
      <Progress value={progress} label={`Tiến độ mẫu cho ${title}`} />
      <StatusBadge tone="warning">{status}</StatusBadge>
    </article>
  );
}
