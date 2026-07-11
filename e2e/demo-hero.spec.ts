import { expect, test } from '@playwright/test'
import {
  consoleCapsule,
  expandDemoViaHero,
  expectPlaybackPlaying,
  gotoHome,
  waitForSimulationTick,
} from './helpers'

test.describe('Hero demo CTA', () => {
  test.beforeEach(async ({ page }) => {
    await gotoHome(page)
  })

  test('scrolls to #demo, expands the console, and starts playback', async ({ page }) => {
    const demoSection = page.locator('#demo')
    await expect(demoSection).toBeVisible()

    await expandDemoViaHero(page)

    await expect(demoSection).toBeInViewport()
    await expect(consoleCapsule(page)).toHaveAttribute('aria-expanded', 'true')
    await expect(page.locator('#demo-workstation-body')).not.toHaveAttribute('hidden')
    await expectPlaybackPlaying(page)

    await page.getByRole('tab', { name: /builder/i }).click()
    await waitForSimulationTick(page)
  })
})
