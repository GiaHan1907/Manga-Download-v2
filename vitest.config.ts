import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["tests/**/*.test.{ts,tsx}"],
    setupFiles: ["./tests/setup.ts"],
    // Every file builds its own jsdom, so a full run runs 20+ environments at
    // once. The same assertions take <1 s alone but were timing out at the 5 s
    // default under that load (a different file each run) — a flaky release
    // gate is worse than a slow one.
    testTimeout: 15_000,
    hookTimeout: 15_000,
    // Each file builds its own jsdom; letting every core run one at once made
    // the slowest assertions time out non-deterministically on a loaded
    // machine. Four at a time keeps a full run reproducible.
    maxWorkers: 4,
  },
});
