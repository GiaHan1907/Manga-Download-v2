import type { ReactNode } from "react";

export type TooltipProps = {
  content: string;
  children: ReactNode;
  /**
   * Set when the trigger is a permanently disabled control: the wrapper
   * becomes keyboard-focusable so `:focus-within` can reveal the tooltip —
   * disabled buttons themselves never receive focus.
   */
  focusableWhenDisabled?: boolean;
};

export default function Tooltip({ content, children, focusableWhenDisabled }: TooltipProps) {
  return (
    <span className="tooltip" tabIndex={focusableWhenDisabled ? 0 : undefined}>
      {children}
      <span className="tooltip__content" role="tooltip">
        {content}
      </span>
    </span>
  );
}
