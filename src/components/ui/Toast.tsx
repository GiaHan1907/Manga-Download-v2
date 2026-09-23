import { useEffect, useState } from "react";
import { getToasts, subscribeToasts, type ToastItem } from "./toastStore";

export default function ToastViewport() {
  const [visible, setVisible] = useState<ToastItem[]>(getToasts());
  useEffect(() => subscribeToasts(setVisible), []);
  if (visible.length === 0) {
    return null;
  }
  return (
    <div className="toast-stack" role="status" aria-live="polite">
      {visible.map((item) => (
        <div key={item.id} className="toast">
          {item.message}
        </div>
      ))}
    </div>
  );
}
