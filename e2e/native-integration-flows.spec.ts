import { expect, test } from '@playwright/test'
import {
  assertNoSeriousAxeViolations,
  gotoNativePreview,
  selectDashboardTab,
  waitForDashboardReady,
} from './helpers/native'

test.describe('Native integration flows', () => {
  test('onboarding consent and detection flow', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard', { skipOnboarding: false })
    await expect(page.getByRole('dialog')).toBeVisible()
    await expect(page.getByText(/documented configuration paths/i)).toBeVisible()

    await page.getByRole('button', { name: /get started/i }).click()
    await expect(page.getByText(/found \d+ configuration path/i)).toBeVisible({ timeout: 10_000 })

    await assertNoSeriousAxeViolations(page, '[role="dialog"]')
  })

  test('integrations panel shows health status and connect actions', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /integrations/i)

    await expect(
      page.getByText(/waiting for first event|action needed|connected/i).first(),
    ).toBeVisible()
    await expect(page.getByRole('button', { name: /^connect$/i }).first()).toBeVisible()
    await expect(page.getByRole('button', { name: /^repair$/i }).first()).toBeVisible()

    await assertNoSeriousAxeViolations(page, '[data-testid="dashboard-shell"]')
  })

  test('connect opens diff review', async ({ page }) => {
    await gotoNativePreview(page, 'dashboard')
    await waitForDashboardReady(page)
    await selectDashboardTab(page, /integrations/i)

    await page
      .getByRole('button', { name: /^connect$/i })
      .first()
      .click()
    await expect(page.getByLabel(/integration diff review/i)).toBeVisible({ timeout: 10_000 })
    await expect(page.getByText(/~\//)).toBeVisible()
  })
})
