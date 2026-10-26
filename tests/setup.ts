import { cleanup, configure } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

// findBy*/waitFor default to a 1 s window, which a full run blows through:
// 20+ jsdom environments compete for the CPU, and a screen that renders in
// 400 ms alone can take longer than a second under that load. The assertion
// still fails on a real timeout — it just stops failing because a sibling
// test file happened to be rendering at the same moment.
configure({ asyncUtilTimeout: 5_000 });

// jsdom lacks the object-URL APIs the offline reader uses for per-page blobs.
if (typeof window !== "undefined") {
  let counter = 0;
  window.URL.createObjectURL = vi.fn(() => {
    counter += 1;
    return `blob:mock-${counter}`;
  }) as unknown as typeof window.URL.createObjectURL;
  window.URL.revokeObjectURL = vi.fn() as unknown as typeof window.URL.revokeObjectURL;
}
