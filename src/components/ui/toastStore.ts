/// UI plan C5: a minimal transient confirmation stack for SUCCESS feedback
/// only (errors keep their inline/alert placement). Deliberately NOT a
/// notification center — messages live ~4s and never stack beyond 3.
/// Store lives apart from the viewport component (react-refresh constraint).

export type ToastItem = { id: number; message: string };

const TOAST_LIFETIME_MS = 4000;
const MAX_VISIBLE = 3;

let nextId = 1;
let items: ToastItem[] = [];
const listeners = new Set<(items: ToastItem[]) => void>();

export function subscribeToasts(listener: (items: ToastItem[]) => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getToasts(): ToastItem[] {
  return [...items];
}

export function pushToast(message: string): void {
  const item: ToastItem = { id: nextId++, message };
  items = [...items, item].slice(-MAX_VISIBLE);
  for (const listener of listeners) {
    listener([...items]);
  }
  window.setTimeout(() => {
    items = items.filter((entry) => entry.id !== item.id);
    for (const listener of listeners) {
      listener([...items]);
    }
  }, TOAST_LIFETIME_MS);
}
