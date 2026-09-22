import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

// jsdom lacks the object-URL APIs the offline reader uses for per-page blobs.
if (typeof window !== "undefined") {
  let counter = 0;
  window.URL.createObjectURL = vi.fn(() => {
    counter += 1;
    return `blob:mock-${counter}`;
  }) as unknown as typeof window.URL.createObjectURL;
  window.URL.revokeObjectURL = vi.fn() as unknown as typeof window.URL.revokeObjectURL;
}
