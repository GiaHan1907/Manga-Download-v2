import type { ReactNode } from "react";
import Progress from "./Progress";
import StatusBadge, { type StatusBadgeTone } from "./StatusBadge";

export type DownloadCardProps = {
  title: string;
  description: string;
  progress: number;
  status?: string;
  statusTone?: StatusBadgeTone;
  children?: ReactNode;
};

export default function DownloadCard({
  title,
  description,
  progress,
  status = "Mẫu trạng thái",
  statusTone = "warning",
  children,
}: DownloadCardProps) {
  return (
    <article className="download-card">
      <div>
        <h3 className="download-card__title">{title}</h3>
        <p className="download-card__meta">{description}</p>
      </div>
      <Progress value={progress} label={`Tiến độ mô phỏng theo trang cho ${title}`} />
      <div className="download-card__footer">
        <StatusBadge tone={statusTone}>{status}</StatusBadge>
        {children && <div className="download-card__actions">{children}</div>}
      </div>
    </article>
  );
}
