import { useSyncExternalStore } from "react";

/// Phase 08 cross-screen refresh (integration-map §2): a tiny external store
/// of per-domain versions. Deliberately NOT a data cache — screens still own
/// their `useAsyncData` fetches; this only answers "when should domain X
/// refetch because something else mutated it?". Chosen over adding
/// TanStack/Zustand because the repo pattern is nonce-in-deps and the plan
/// forbids stack churn ("existing structure wins").
export type InvalidationDomain =
  | "home"
  | "library"
  | "favorites"
  | "downloads"
  | "settings"
  | "sources"
  | "history";

const versions = new Map<InvalidationDomain, number>();
const listeners = new Set<() => void>();

export function bumpDomain(domain: InvalidationDomain): void {
  versions.set(domain, (versions.get(domain) ?? 0) + 1);
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/// Returns a number that changes whenever `domain` was bumped — put it in a
/// `useAsyncData` deps array alongside the screen's own inputs.
export function useDomainVersion(domain: InvalidationDomain): number {
  return useSyncExternalStore(subscribe, () => versions.get(domain) ?? 0);
}
