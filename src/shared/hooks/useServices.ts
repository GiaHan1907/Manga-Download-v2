import { useSyncExternalStore } from "react";
import { getServicesVersion, subscribeServices } from "../../services/provider";

export function useServicesVersion(): number {
  return useSyncExternalStore(subscribeServices, getServicesVersion, getServicesVersion);
}
