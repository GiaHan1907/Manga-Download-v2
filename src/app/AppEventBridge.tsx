import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { subscribeDownloadTask, subscribeTrayOpenDownloads } from "../shared/api/desktop";
import { applyDownloadEventToInvalidation } from "./downloadInvalidation";

/// Single app-level listener (mounted once in AppShell): download events
/// fan out as domain bumps, and tray "Open Downloads" navigates.
export default function AppEventBridge() {
  const navigate = useNavigate();

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const attach = (subscription: Promise<() => void>) => {
      // Outside Tauri (plain browser/vitest without event mocks) subscribing
      // can reject — the bridge is an enhancement, never a startup blocker.
      subscription
        .then((unlisten) => {
          if (disposed) {
            unlisten();
          } else {
            unlisteners.push(unlisten);
          }
        })
        .catch(() => undefined);
    };
    try {
      attach(
        subscribeDownloadTask((event) => {
          if (!disposed) {
            applyDownloadEventToInvalidation(event);
          }
        }),
      );
      attach(
        subscribeTrayOpenDownloads(() => {
          if (!disposed) {
            navigate("/downloads");
          }
        }),
      );
    } catch {
      /* non-Tauri environment */
    }
    return () => {
      disposed = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [navigate]);

  return null;
}
