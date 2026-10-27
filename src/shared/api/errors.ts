import { normalizeIpcError } from "./desktop";

/// Services turn a structured IPC error into a plain `Error` so screens can
/// read `error.message` without knowing the IPC shape. The structured
/// { code, message } pair is still available at the boundary — screens that
/// need to branch on the code call `normalizeIpcError` on the caught value.
export function rethrowIpc(err: unknown): never {
  throw new Error(normalizeIpcError(err).message);
}

/// True when a key event comes from a field the user is typing into, or (with
/// `includeButtons`, which the reader needs) from any focused control whose own
/// activation would otherwise double-fire the shortcut.
export function isTypingTarget(
  target: EventTarget | null,
  includeButtons = false,
): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable) {
    return true;
  }
  return includeButtons && tag === "BUTTON";
}
