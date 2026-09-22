import { X } from "lucide-react";
import { useEffect, type ReactNode } from "react";
import Button from "./Button";

export type DialogProps = {
  open: boolean;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
};

export default function Dialog({ open, title, description, children, footer, onClose }: DialogProps) {
  useEffect(() => {
    if (!open) {
      return undefined;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose, open]);

  if (!open) {
    return null;
  }

  return (
    <div className="dialog-backdrop" onMouseDown={onClose}>
      <section
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title"
        aria-describedby={description ? "dialog-description" : undefined}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="dialog__header">
          <div>
            <h2 id="dialog-title">{title}</h2>
            {description && (
              <p id="dialog-description" className="muted">
                {description}
              </p>
            )}
          </div>
          <Button variant="ghost" iconOnly aria-label="Đóng hộp thoại" onClick={onClose}>
            <X size={18} aria-hidden="true" />
          </Button>
        </header>
        <div className="dialog__body">{children}</div>
        {footer && <footer className="dialog__footer">{footer}</footer>}
      </section>
    </div>
  );
}
