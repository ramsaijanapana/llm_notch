import { expect, test } from '@playwright/test'
import {
  gotoNativePreview,
  selectDashboardTab,
  waitForDashboardReady,
  waitForOverlayReady,
} from './helpers/native'

test.describe('Native reduced motion', () => {
  test('marks overlay and dashboard shells when reduce motion is enabled in settings', async ({
    page,
  }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await expect(page.getByTestId('overlay-shell')).toHaveAttribute('data-reduced-motion', 'false')

    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /settings/i)
    await page.getByLabel(/reduce motion/i).check()

    await expect(
      page.locator('[data-surface="dashboard"][data-reduced-motion="true"]').first(),
    ).toBeVisible()
  })
})
