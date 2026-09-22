export type StatusBadgeTone = "success" | "warning" | "error" | "neutral";

export type StatusBadgeProps = {
  children: string;
  tone?: StatusBadgeTone;
};

export default function StatusBadge({ children, tone = "neutral" }: StatusBadgeProps) {
  return <span className={`status-badge status-badge--${tone}`}>{children}</span>;
}
