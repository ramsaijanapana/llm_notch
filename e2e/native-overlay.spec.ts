import { expect, test } from '@playwright/test'
import {
  assertNoApproveDenyControls,
  gotoNativePreview,
  openOverlayPeek,
  waitForOverlayReady,
} from './helpers/native'

test.describe('Native overlay surface', () => {
  test('renders compact island with session dots and CPU metrics', async ({ page }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)

    await expect(page.getByTestId('preview-badge')).toHaveText('Preview')
    await expect(page.getByTestId('session-dot-sess-cursor-refactor')).toBeVisible()
    await expect(page.getByTestId('session-dot-sess-claude-review')).toBeVisible()
    await expect(page.getByTestId('compact-cpu')).not.toHaveText('—')
    await expect(page.getByTestId('cpu-sparkline')).toBeVisible()
    await assertNoApproveDenyControls(page)
  })

  test('expands peek with attention queue, footer metrics, and callbacks', async ({ page }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await openOverlayPeek(page)

    await expect(page.getByTestId('attention-section')).toBeVisible()
    await expect(page.getByTestId('session-row-sess-cursor-refactor')).toBeVisible()
    await expect(page.getByTestId('session-row-sess-claude-review')).toBeVisible()
    await expect(page.getByTestId('footer-cpu')).toContainText('CPU')
    await expect(page.getByTestId('footer-rss')).toContainText('RSS')
    await expect(page.getByTestId('footer-read')).toContainText('Read')
    await expect(page.getByTestId('footer-write')).toContainText('Write')
    await expect(page.getByTestId('footer-processes')).toContainText('Processes')
    await expect(page.getByTestId('quality-note')).toBeVisible()
    await expect(page.getByRole('button', { name: /open dashboard/i })).toBeVisible()
    await expect(
      page.getByRole('button', { name: /acknowledge review overlay accessibility/i }),
    ).toBeVisible()
    await assertNoApproveDenyControls(page)
  })

  test('acknowledge clears local attention in peek', async ({ page }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await openOverlayPeek(page)

    await page.getByRole('button', { name: /acknowledge review overlay accessibility/i }).click()
    await expect(page.getByTestId('attention-section')).toHaveCount(0)
  })

  test('propagates live stream metrics into peek session rows', async ({ page }) => {
    await gotoNativePreview(page, 'overlay')
    await waitForOverlayReady(page)
    await openOverlayPeek(page)

    const sessionRow = page.getByTestId('session-row-sess-cursor-refactor')
    const initial = await sessionRow.textContent()

    await expect
      .poll(async () => sessionRow.textContent(), {
        timeout: 15_000,
        message: 'Expected preview stream metrics to update peek session row',
      })
      .not.toBe(initial)
  })

  test('shows resync stale banner in peek via preview bridge', async ({ page }) => {
    await gotoNativePreview(page, 'overlay', { scenario: 'resync' })
    await waitForOverlayReady(page)
    await openOverlayPeek(page)
    await expect(page.getByTestId('connection-banner')).toContainText(
      /preview stream requested resync/i,
    )
  })
})
