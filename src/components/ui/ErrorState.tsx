import type { ReactNode } from "react";
import { TriangleAlert } from "lucide-react";
import Button from "./Button";

export type ErrorStateProps = {
  title: string;
  description: string;
  action?: ReactNode;
  /** Convenience standard retry button (UI plan C5). Custom `action` wins. */
  onRetry?: () => void;
  retryLabel?: string;
};

export default function ErrorState({ title, description, action, onRetry, retryLabel = "Thử lại" }: ErrorStateProps) {
  return (
    <section className="error-state" role="alert">
      <div className="error-state__icon">
        <TriangleAlert size={22} aria-hidden="true" />
      </div>
      <div>
        <h2>{title}</h2>
        <p className="muted">{description}</p>
      </div>
      {action ?? (onRetry && <Button variant="primary" onClick={onRetry}>{retryLabel}</Button>)}
    </section>
  );
}
