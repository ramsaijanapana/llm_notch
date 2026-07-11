import { expect, test } from '@playwright/test'
import {
  assertNoSeriousAxeViolations,
  consoleCapsule,
  ensureDemoExpanded,
  gotoHome,
  resetSimulation,
} from './helpers'

test.describe('Accessibility scans', () => {
  test('initial page has no serious or critical axe violations', async ({ page }) => {
    await gotoHome(page)
    await assertNoSeriousAxeViolations(page)
  })

  test('expanded demo has no serious or critical axe violations', async ({ page }) => {
    await gotoHome(page)
    await ensureDemoExpanded(page)
    await assertNoSeriousAxeViolations(page, '#demo')
  })

  test('reset leaves focus on the collapsed console capsule', async ({ page }) => {
    await gotoHome(page)
    await ensureDemoExpanded(page)

    await resetSimulation(page, { keepCollapsed: true })

    const capsule = consoleCapsule(page)
    await expect(capsule).toHaveAttribute('aria-expanded', 'false')
    await expect(capsule).toBeFocused()
  })
})
