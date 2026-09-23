import type { ReactNode } from "react";
import Progress from "./Progress";
import StatusBadge, { type StatusBadgeTone } from "./StatusBadge";
import { sparklinePoints } from "../../types/downloads";

export type DownloadCardProps = {
  title: string;
  description: string;
  progress: number;
  status?: string;
  statusTone?: StatusBadgeTone;
  /** Recent measured speeds (bytes/s, oldest first) — renders a sparkline
   *  when ≥2 samples carry a shape. */
  speedHistory?: number[];
  /** Rejection of the last user action on THIS task; inline so feedback
   *  appears on the card that was clicked, not at the top of the page. */
  actionError?: string;
  children?: ReactNode;
};

export default function DownloadCard({
  title,
  description,
  progress,
  status = "Trạng thái",
  statusTone = "warning",
  speedHistory,
  actionError,
  children,
}: DownloadCardProps) {
  const points = speedHistory && speedHistory.length >= 2 ? sparklinePoints(speedHistory) : null;
  return (
    <article className="download-card">
      <div>
        <h3 className="download-card__title">{title}</h3>
        <p className="download-card__meta">{description}</p>
      </div>
      {points && (
        <svg
          className="download-card__sparkline"
          viewBox="0 0 100 28"
          width="100"
          height="28"
          role="img"
          aria-label="Biểu đồ tốc độ tải gần đây"
        >
          <polyline points={points} fill="none" stroke="var(--primary)" strokeWidth="1.5" />
        </svg>
      )}
      <Progress value={progress} label={`Tiến độ theo trang cho ${title}`} />
      {actionError && (
        <p className="download-card__error" role="alert">
          {actionError}
        </p>
      )}
      <div className="download-card__footer">
        <StatusBadge tone={statusTone}>{status}</StatusBadge>
        {children && <div className="download-card__actions">{children}</div>}
      </div>
    </article>
  );
}
