import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  timeout: 30_000,
  expect: {
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.01
    }
  },
  use: {
    trace: "on-first-retry"
  },
  projects: [
    {
      name: "desktop",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1535, height: 786 } }
    },
    {
      name: "narrow",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1180, height: 760 } }
    }
  ]
});
