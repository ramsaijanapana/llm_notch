import { expect, test } from '@playwright/test'
import { gotoNativePreview, waitForDashboardReady } from './helpers/native'

test.describe('Native connection states', () => {
  test('shows loading banner before bootstrap completes', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', {
      scenario: 'loading',
      waitUntil: 'commit',
    })

    await expect(page.getByText(/loading native snapshot/i)).toBeVisible()
    await waitForDashboardReady(page)
    await expect(page.getByText(/loading native snapshot/i)).toHaveCount(0)
  })

  test('shows disconnected state', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { scenario: 'disconnected' })
    await expect(page.getByText(/preview host disconnected/i).first()).toBeVisible()
  })

  test('shows resyncing state then recovers', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { scenario: 'resync' })
    await expect(page.getByText(/resyncing native stream/i)).toBeVisible()
    await waitForDashboardReady(page)
    await expect(page.getByText(/resyncing native stream/i)).toHaveCount(0)
  })

  test('shows incompatible protocol state', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { scenario: 'incompatible' })
    await expect(page.getByText(/incompatible with renderer protocol/i).first()).toBeVisible()
  })
})
