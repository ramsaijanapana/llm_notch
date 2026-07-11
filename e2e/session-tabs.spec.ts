import { expect, test } from '@playwright/test'
import { ensureDemoExpanded, gotoHome, resetSimulation } from './helpers'

test.describe('Session tabs keyboard navigation', () => {
  test.beforeEach(async ({ page }) => {
    await gotoHome(page)
    await ensureDemoExpanded(page)
  })

  test.afterEach(async ({ page }) => {
    await resetSimulation(page)
  })

  test('supports ArrowRight, Home, and End and references tabpanels', async ({ page }) => {
    const tabs = page.getByRole('tab')
    await expect(tabs).toHaveCount(4)

    for (const tab of await tabs.all()) {
      const panelId = await tab.getAttribute('aria-controls')
      expect(panelId).toBeTruthy()
      await expect(page.locator(`#${panelId}`)).toHaveAttribute('role', 'tabpanel')
    }

    const builderTab = page.getByRole('tab', { name: /builder/i })
    const testerTab = page.getByRole('tab', { name: /tester/i })
    const writerTab = page.getByRole('tab', { name: /writer/i })

    await builderTab.focus()

    await page.keyboard.press('ArrowRight')
    await expect(testerTab).toHaveAttribute('aria-selected', 'true')
    await expect(testerTab).toBeFocused()
    await expect(page.locator('#session-panel-tester')).not.toHaveAttribute('hidden')

    await page.keyboard.press('Home')
    await expect(builderTab).toHaveAttribute('aria-selected', 'true')
    await expect(builderTab).toBeFocused()
    await expect(page.locator('#session-panel-builder')).not.toHaveAttribute('hidden')

    await page.keyboard.press('End')
    await expect(writerTab).toHaveAttribute('aria-selected', 'true')
    await expect(writerTab).toBeFocused()
    await expect(page.locator('#session-panel-writer')).not.toHaveAttribute('hidden')
  })
})
