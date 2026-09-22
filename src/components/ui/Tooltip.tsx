import type { ReactNode } from "react";

export type TooltipProps = {
  content: string;
  children: ReactNode;
};

export default function Tooltip({ content, children }: TooltipProps) {
  return (
    <span className="tooltip">
      {children}
      <span className="tooltip__content" role="tooltip">
        {content}
      </span>
    </span>
  );
}
