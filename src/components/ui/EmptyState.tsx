import type { ReactNode } from "react";
import { Inbox } from "lucide-react";

export type EmptyStateProps = {
  title: string;
  description: string;
  action?: ReactNode;
};

export default function EmptyState({ title, description, action }: EmptyStateProps) {
  return (
    <section className="empty-state">
      <div className="empty-state__icon">
        <Inbox size={22} aria-hidden="true" />
      </div>
      <div>
        <h2>{title}</h2>
        <p className="muted">{description}</p>
      </div>
      {action}
    </section>
  );
}
