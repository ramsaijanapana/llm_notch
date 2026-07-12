import { expect, test } from '@playwright/test'
import { gotoNativePreview, selectDashboardTab, waitForDashboardReady } from './helpers/native'

test.describe('Native dashboard surface', () => {
  test('navigates Sessions, Metrics, Integrations, and Settings tabs', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)

    await expect(page.getByRole('region', { name: 'Attention queue' })).toBeVisible()
    await expect(page.getByRole('region', { name: 'Active sessions' })).toBeVisible()

    await selectDashboardTab(page, /metrics/i)
    await expect(page.getByRole('table')).toBeVisible()
    await expect(page.getByRole('button', { name: /15 minutes/i })).toHaveAttribute(
      'aria-pressed',
      'true',
    )

    await selectDashboardTab(page, /integrations/i)
    await expect(page.getByText(/llm_notch entries/i).first()).toBeVisible()

    await selectDashboardTab(page, /settings/i)
    await expect(page.getByLabel(/show island overlay/i)).toBeVisible()
    await expect(page.getByLabel(/^display$/i)).toBeVisible()
  })

  test('supports keyboard shortcuts when onboarding is closed', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)

    await page.keyboard.press('Control+2')
    await expect(page.getByRole('tab', { name: /metrics/i })).toHaveAttribute(
      'aria-selected',
      'true',
    )

    await page.keyboard.press('Meta+3')
    await expect(page.getByRole('tab', { name: /integrations/i })).toHaveAttribute(
      'aria-selected',
      'true',
    )

    await page.keyboard.press('Control+4')
    await expect(page.getByRole('tab', { name: /settings/i })).toHaveAttribute(
      'aria-selected',
      'true',
    )
  })

  test('onboarding focuses display and suppresses tab shortcuts', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { skipOnboarding: false })
    await expect(page.getByRole('dialog')).toBeVisible()
    await expect
      .poll(async () => page.evaluate(() => document.activeElement?.id ?? ''))
      .toBe('onboarding-display')

    await page.keyboard.press('Control+2')
    await expect(page.getByRole('dialog')).toBeVisible()
    await expect(page.getByRole('tab', { name: /metrics/i })).toHaveCount(0)
  })

  test('Escape toggles skip confirmation during onboarding', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { skipOnboarding: false })
    await expect(page.getByRole('dialog')).toBeVisible()

    await page.keyboard.press('Escape')
    await expect(page.getByRole('alertdialog', { name: /confirm skip setup/i })).toBeVisible()
    await expect(page.getByRole('button', { name: /continue setup/i })).toBeFocused()

    await page.keyboard.press('Escape')
    await expect(page.getByRole('alertdialog', { name: /confirm skip setup/i })).toHaveCount(0)
  })

  test('shows automatic display option and Windows fullscreen messaging', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /settings/i)

    await expect(page.getByLabel(/^display$/i)).toContainText(/automatic/i)

    await page.addInitScript(() => {
      Object.defineProperty(navigator, 'userAgent', {
        value:
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        configurable: true,
      })
    })
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /settings/i)

    await expect(page.getByText(/fullscreen overlay is unavailable on windows/i)).toBeVisible()
    await expect(page.getByLabel(/show over fullscreen apps/i)).toBeDisabled()
  })

  test('propagates stream metrics into session detail', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)

    const cpuMetric = page
      .getByRole('group', { name: /current session metrics/i })
      .getByText(/%/i)
      .first()
    const initial = await cpuMetric.textContent()

    await expect
      .poll(async () => cpuMetric.textContent(), {
        timeout: 15_000,
        message: 'Expected live preview metrics to update session detail',
      })
      .not.toBe(initial)
  })

  test('renders 15m live and persisted 1h/24h history with coverage labels', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /metrics/i)

    await expect(page.getByTestId('host-history-coverage')).toContainText(/of selected 15 minutes/i)

    await page.getByRole('button', { name: /1 hour/i }).click()
    await expect(page.getByTestId('host-history-coverage')).toContainText(/of selected 1 hour/i)

    await page.getByRole('button', { name: /24 hours/i }).click()
    await expect(page.getByTestId('aggregate-history-coverage')).toContainText(
      /of selected 24 hours/i,
    )

    await expect(page.getByText(/Cursor — Refactor telemetry bridge/i).first()).toBeVisible()
    await expect(
      page.getByText(/Claude Code — Review overlay accessibility/i).first(),
    ).toBeVisible()
  })

  test('disables impossible history ranges based on retention', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /settings/i)

    await page.getByLabel(/history retention/i).selectOption('1')
    await selectDashboardTab(page, /metrics/i)

    await expect(page.getByRole('button', { name: /24 hours/i })).toBeDisabled()
    await expect(page.getByRole('button', { name: /1 hour/i })).toBeEnabled()
  })
})
