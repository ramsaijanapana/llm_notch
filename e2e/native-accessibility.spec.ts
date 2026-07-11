import { test } from '@playwright/test'
import {
  assertNoSeriousAxeViolations,
  gotoNativePreview,
  openOverlayPeek,
  selectDashboardTab,
  waitForDashboardReady,
  waitForOverlayReady,
} from './helpers/native'

test.describe('Native accessibility scans', () => {
  test('overlay compact has no serious or critical axe violations', async ({ page }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await assertNoSeriousAxeViolations(page, '[data-testid="compact-island"]')
  })

  test('overlay peek has no serious or critical axe violations', async ({ page }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await openOverlayPeek(page)
    await assertNoSeriousAxeViolations(page, '[data-testid="peek-panel"]')
  })

  test('onboarding dialog has no serious or critical axe violations', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { skipOnboarding: false })
    await assertNoSeriousAxeViolations(page, '[role="dialog"]')
  })

  test('dashboard panels have no serious or critical axe violations', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)

    for (const tab of [/sessions/i, /metrics/i, /integrations/i, /settings/i] as const) {
      await selectDashboardTab(page, tab)
      await assertNoSeriousAxeViolations(page, '[data-testid="dashboard-shell"]')
    }
  })
})
