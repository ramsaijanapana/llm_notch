import { test } from '@playwright/test'
import {
  assertNoDocumentHorizontalOverflow,
  gotoNativePreview,
  waitForDashboardReady,
  waitForOverlayReady,
} from './helpers/native'

const overlayViewports = [
  { width: 360, height: 120 },
  { width: 420, height: 140 },
  { width: 520, height: 160 },
] as const

const dashboardViewports = [
  { width: 720, height: 900 },
  { width: 900, height: 900 },
  { width: 1440, height: 900 },
] as const

test.describe('Native layout overflow', () => {
  for (const viewport of overlayViewports) {
    test(`overlay has no horizontal overflow at ${viewport.width}px`, async ({ page }) => {
      await page.setViewportSize(viewport)
      await gotoNativePreview(page, 'overlay')
      await waitForOverlayReady(page)
      await assertNoDocumentHorizontalOverflow(page)
    })
  }

  for (const viewport of dashboardViewports) {
    test(`dashboard has no horizontal overflow at ${viewport.width}px`, async ({ page }) => {
      await page.setViewportSize(viewport)
      await gotoNativePreview(page, 'dashboard')
      await waitForDashboardReady(page)
      await assertNoDocumentHorizontalOverflow(page)
    })
  }
})
