import { expect, test } from '@playwright/test'
import {
  assertMarketingHome,
  gotoNativePreview,
  selectDashboardTab,
  trackCrossOriginRequests,
  waitForDashboardReady,
  waitForOverlayReady,
} from './helpers/native'

test.describe('Native runtime network isolation', () => {
  test('marketing home stays separate from native preview routing', async ({ page }) => {
    await assertMarketingHome(page)
  })

  test('native overlay makes no cross-origin runtime requests', async ({ page }) => {
    const tracker = trackCrossOriginRequests(page)

    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await page.locator('[data-native-overlay-container]').hover()
    await page.getByRole('button', { name: /open dashboard/i }).click()
    await page.getByRole('button', { name: /acknowledge review overlay accessibility/i }).click()

    tracker.stop()
    expect(tracker.getCrossOriginRequests()).toEqual([])
  })

  test('native dashboard makes no cross-origin runtime requests', async ({ page }) => {
    const tracker = trackCrossOriginRequests(page)

    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)

    await selectDashboardTab(page, /metrics/i)
    await page.getByRole('button', { name: /1 hour/i }).click()
    await selectDashboardTab(page, /integrations/i)
    await page
      .getByRole('button', { name: /^preview$/i })
      .first()
      .click()
    await selectDashboardTab(page, /settings/i)
    await page.getByLabel(/show island overlay/i).click()

    tracker.stop()
    expect(tracker.getCrossOriginRequests()).toEqual([])
  })
})
