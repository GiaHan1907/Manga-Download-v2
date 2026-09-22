import type { ReactNode } from "react";
import { TriangleAlert } from "lucide-react";

export type ErrorStateProps = {
  title: string;
  description: string;
  action?: ReactNode;
};

export default function ErrorState({ title, description, action }: ErrorStateProps) {
  return (
    <section className="error-state" role="alert">
      <div className="error-state__icon">
        <TriangleAlert size={22} aria-hidden="true" />
      </div>
      <div>
        <h2>{title}</h2>
        <p className="muted">{description}</p>
      </div>
      {action}
    </section>
  );
}
