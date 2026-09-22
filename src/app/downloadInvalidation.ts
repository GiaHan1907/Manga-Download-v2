import { bumpDomain, type InvalidationDomain } from "../shared/hooks/useInvalidation";
import type { DownloadTaskEvent } from "../types/downloads";

/// Which screens must refetch when a task event arrives. Events never
/// replace snapshots — they only invalidate; each screen refetches from
/// Rust. Unknown/intermediate statuses intentionally bump nothing.
export function domainsForTaskStatus(status: string): InvalidationDomain[] {
  switch (status) {
    case "completed":
      return ["downloads", "library", "home"];
    case "failed":
    case "cancelled":
      return ["downloads", "home"];
    case "queued":
    case "resolving":
    case "downloading":
    case "verifying":
    case "exporting":
    case "paused":
    case "pausing":
      return ["downloads"];
    default:
      return [];
  }
}

/// Pure handler used by tests: apply one event to the invalidation bus.
export function applyDownloadEventToInvalidation(event: DownloadTaskEvent): void {
  for (const domain of domainsForTaskStatus(event.task.status)) {
    bumpDomain(domain);
  }
}
