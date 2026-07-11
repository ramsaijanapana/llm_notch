import { expect, test } from '@playwright/test'
import { demoStatus, ensureDemoExpanded, gotoHome, resetSimulation } from './helpers'

test.describe('Simulated terminal drawer', () => {
  test.beforeEach(async ({ page }) => {
    await gotoHome(page)
    await ensureDemoExpanded(page)
  })

  test.afterEach(async ({ page }) => {
    await resetSimulation(page)
  })

  test('opens with simulation-only labeling, closes, and restores Jump focus', async ({ page }) => {
    const jumpButton = page.getByRole('button', { name: /jump to workspace/i })

    await jumpButton.click()

    const terminal = page.getByRole('complementary', { name: /simulated terminal/i })
    await expect(terminal).toBeVisible()
    await expect(terminal.getByText('Simulation only', { exact: true })).toBeVisible()
    await expect(terminal).toContainText(/no shell, network, or filesystem access/i)
    await expect(demoStatus(page)).toContainText(/terminal focus requested/i)

    await page.getByRole('button', { name: /close simulated terminal/i }).click()

    await expect(page.getByRole('complementary', { name: /simulated terminal/i })).toHaveCount(0)
    await expect(jumpButton).toBeFocused()
  })
})
