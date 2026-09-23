import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { subscribeDownloadTask, subscribeLibraryAlerts, subscribeTrayOpenDownloads } from "../shared/api/desktop";
import { applyDownloadEventToInvalidation } from "./downloadInvalidation";
import { bumpDomain } from "../shared/hooks/useInvalidation";

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
      // Feature plan F1: watcher tick found new chapters — Library/Home
      // re-snapshot (counts come from library_list; no payload data needed).
      attach(
        subscribeLibraryAlerts(() => {
          if (!disposed) {
            bumpDomain("library");
            bumpDomain("home");
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
