import { expect, test } from '@playwright/test'
import {
  demoStatus,
  ensureDemoExpanded,
  gotoHome,
  resetSimulation,
  selectSessionTab,
} from './helpers'

test.describe('Decision panel flows', () => {
  test.beforeEach(async ({ page }) => {
    await gotoHome(page)
    await ensureDemoExpanded(page)
  })

  test.afterEach(async ({ page }) => {
    await resetSimulation(page)
  })

  test('tester approval and reject/retry flows work', async ({ page }) => {
    await selectSessionTab(page, /tester/i)

    await expect(page.getByText(/approve running: npm test --coverage/i)).toBeVisible()

    await page.getByRole('button', { name: 'Approve' }).click()
    await expect(page.getByRole('heading', { name: /running/i })).toBeVisible()
    await expect(demoStatus(page)).toContainText(/tester approved/i)

    await resetSimulation(page)
    await selectSessionTab(page, /tester/i)

    await page.getByRole('button', { name: 'Reject' }).click()
    await expect(page.getByRole('heading', { name: /paused/i })).toBeVisible()
    await expect(page.getByText(/permission denied by operator/i)).toBeVisible()
    await expect(demoStatus(page)).toContainText(/tester rejected/i)

    await page.getByRole('button', { name: 'Retry' }).click()
    await expect(page.getByRole('heading', { name: /running/i })).toBeVisible()
    await expect(demoStatus(page)).toContainText(/retry requested/i)
  })

  test('reviewer blank validation and valid answer flow work', async ({ page }) => {
    await selectSessionTab(page, /reviewer/i)

    await page.getByRole('button', { name: /submit answer/i }).click()
    await expect(page.getByRole('alert')).toHaveText(/enter an answer before submitting/i)

    await page.getByLabel(/your answer/i).fill('Use { error: string } for consistency.')
    await page.getByRole('button', { name: /submit answer/i }).click()

    await expect(page.getByRole('heading', { name: /running/i })).toBeVisible()
    await expect(demoStatus(page)).toContainText(/reviewer answered/i)
    await expect(page.getByRole('log', { name: /session event log/i })).toContainText(
      /use \{ error: string \}/i,
    )
  })
})
