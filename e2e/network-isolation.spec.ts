import { expect, test } from '@playwright/test'
import { ensureDemoExpanded, gotoHome, trackCrossOriginRequests } from './helpers'

test.describe('Runtime network isolation', () => {
  test('makes no cross-origin runtime requests', async ({ page }) => {
    const tracker = trackCrossOriginRequests(page)

    await gotoHome(page)
    await ensureDemoExpanded(page)

    await page.getByRole('tab', { name: /tester/i }).click()
    await page.getByRole('button', { name: 'Approve' }).click()
    await page.getByRole('tab', { name: /reviewer/i }).click()
    await page.getByLabel(/your answer/i).fill('Local-only response')
    await page.getByRole('button', { name: /submit answer/i }).click()
    await page.getByRole('button', { name: /jump to workspace/i }).click()
    await page.getByRole('button', { name: /close simulated terminal/i }).click()

    tracker.stop()

    expect(
      tracker.getCrossOriginRequests(),
      `Unexpected cross-origin requests: ${tracker.getCrossOriginRequests().join(', ')}`,
    ).toEqual([])
  })
})
