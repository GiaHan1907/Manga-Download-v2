import { useId, type ReactNode } from "react";

export type TooltipProps = {
  content: string;
  children: ReactNode;
  /**
   * Set when the trigger is a permanently disabled control: the wrapper
   * becomes keyboard-focusable so `:focus-within` can reveal the tooltip —
   * disabled buttons themselves never receive focus. The tooltip text is
   * programmatically associated via aria-describedby (a bare role="tooltip"
   * is not announced).
   */
  focusableWhenDisabled?: boolean;
};

export default function Tooltip({ content, children, focusableWhenDisabled }: TooltipProps) {
  const tooltipId = useId();
  return (
    <span
      className="tooltip"
      tabIndex={focusableWhenDisabled ? 0 : undefined}
      aria-describedby={focusableWhenDisabled ? tooltipId : undefined}
    >
      {children}
      <span className="tooltip__content" role="tooltip" id={tooltipId}>
        {content}
      </span>
    </span>
  );
}
