import { expect, test } from '@playwright/test'
import {
  consoleCapsule,
  expandDemoViaHero,
  expectPlaybackPaused,
  gotoHome,
  resetSimulation,
} from './helpers'

test.describe('Reduced motion playback', () => {
  test('keeps playback paused after hero CTA and after reset', async ({ page }) => {
    await gotoHome(page, { reducedMotion: true })

    await expandDemoViaHero(page)

    await expect(consoleCapsule(page)).toHaveAttribute('aria-expanded', 'true')
    await expectPlaybackPaused(page)
    await expect(consoleCapsule(page)).toHaveAttribute('aria-label', /paused/i)

    await resetSimulation(page)

    await expectPlaybackPaused(page)
    await expect(consoleCapsule(page)).toHaveAttribute('aria-label', /paused/i)
  })
})
