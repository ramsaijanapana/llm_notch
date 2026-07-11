import { defineConfig, devices } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://127.0.0.1:4173'

const marketingProjects = [
  {
    name: 'chromium',
    testIgnore: /native-.*\.spec\.ts/,
    use: { ...devices['Desktop Chrome'] },
  },
  {
    name: 'webkit',
    testIgnore: /native-.*\.spec\.ts/,
    use: { ...devices['Desktop Safari'] },
  },
]

const nativeProjects = [
  {
    name: 'native-chromium',
    testMatch: /native-.*\.spec\.ts/,
    use: { ...devices['Desktop Chrome'] },
  },
  {
    name: 'native-webkit',
    testMatch: /native-.*\.spec\.ts/,
    use: { ...devices['Desktop Safari'] },
  },
]

const firefoxProject =
  process.env.PLAYWRIGHT_ENABLE_FIREFOX === '1'
    ? [
        {
          name: 'firefox',
          testIgnore: /native-.*\.spec\.ts/,
          use: { ...devices['Desktop Firefox'] },
          timeout: 30_000,
        },
        {
          name: 'native-firefox',
          testMatch: /native-.*\.spec\.ts/,
          use: { ...devices['Desktop Firefox'] },
          timeout: 30_000,
        },
      ]
    : []

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 1,
  workers: process.env.CI ? 1 : 2,
  timeout: 45_000,
  expect: { timeout: 10_000 },
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never' }]]
    : [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [...marketingProjects, ...nativeProjects, ...firefoxProject],
  webServer: {
    command: 'npm run build && npm run preview -- --host 127.0.0.1 --port 4173',
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
})
