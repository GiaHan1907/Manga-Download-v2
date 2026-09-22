import { useEffect, useState } from "react";

export type AsyncData<T> =
  | { state: "loading" }
  | { state: "error"; error: unknown }
  | { state: "success"; data: T };

export function useAsyncData<T>(fn: () => Promise<T>, deps: unknown[]): AsyncData<T> {
  const [result, setResult] = useState<AsyncData<T>>({ state: "loading" });

  useEffect(() => {
    let alive = true;
    fn()
      .then((data) => {
        if (alive) {
          setResult({ state: "success", data });
        }
      })
      .catch((error: unknown) => {
        if (alive) {
          setResult({ state: "error", error });
        }
      });
    return () => {
      alive = false;
    };
    // fn is recreated each render; deps drive refetching by design.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return result;
}
