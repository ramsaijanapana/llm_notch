import { test } from '@playwright/test'
import { assertNoDocumentHorizontalOverflow, gotoHome } from './helpers'

const viewports = [
  { width: 320, height: 720 },
  { width: 390, height: 844 },
  { width: 768, height: 1024 },
  { width: 1440, height: 900 },
] as const

test.describe('Document horizontal overflow', () => {
  for (const viewport of viewports) {
    test(`has no horizontal overflow at ${viewport.width}px`, async ({ page }) => {
      await page.setViewportSize(viewport)
      await gotoHome(page)
      await assertNoDocumentHorizontalOverflow(page)
    })
  }
})
