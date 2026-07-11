import { expect, type Page } from '@playwright/test'
import {
  assertNoDocumentHorizontalOverflow,
  assertNoSeriousAxeViolations,
  trackCrossOriginRequests,
} from '../helpers'

export type NativePreviewSurface = 'overlay' | 'dashboard'

const ONBOARDING_KEY = 'llm-notch:onboarding-complete:v1'

export async function prepareNativePreviewPage(
  page: Page,
  options?: { reducedMotion?: boolean; skipOnboarding?: boolean },
) {
  if (options?.reducedMotion) {
    await page.emulateMedia({ reducedMotion: 'reduce' })
  } else {
    await page.emulateMedia({ reducedMotion: 'no-preference' })
  }

  if (options?.skipOnboarding !== false) {
    await page.addInitScript((key) => {
      localStorage.setItem(key, 'true')
    }, ONBOARDING_KEY)
  }
}

export function nativePreviewPath(
  surface: NativePreviewSurface,
  query: Record<string, string> = {},
): string {
  const params = new URLSearchParams({ nativePreview: surface, ...query })
  return `/?${params.toString()}`
}

export async function gotoNativePreview(
  page: Page,
  surface: NativePreviewSurface,
  options?: {
    reducedMotion?: boolean
    skipOnboarding?: boolean
    scenario?: string
    waitUntil?: 'commit' | 'domcontentloaded' | 'load' | 'networkidle'
  },
) {
  await prepareNativePreviewPage(page, options)

  const query: Record<string, string> = {}
  if (options?.scenario) {
    query.nativeScenario = options.scenario
  }

  await page.goto(nativePreviewPath(surface, query), {
    waitUntil: options?.waitUntil ?? 'domcontentloaded',
  })
}

export async function waitForOverlayReady(page: Page) {
  await expect(page.getByTestId('overlay-shell')).toBeVisible()
  await expect(page.getByTestId('compact-island')).toBeVisible()
}

export async function waitForDashboardReady(page: Page) {
  await expect(page.getByTestId('dashboard-shell')).toBeVisible()
  await expect(page.getByRole('tab', { name: /sessions/i })).toBeVisible()
}

export async function openOverlayPeek(page: Page) {
  await page.locator('[data-native-overlay-container]').hover()
  await expect(page.getByTestId('peek-panel')).toBeVisible()
}

export async function selectDashboardTab(page: Page, name: RegExp | string) {
  await page.getByRole('tab', { name }).click()
  await expect(page.getByRole('tab', { name })).toHaveAttribute('aria-selected', 'true')
}

export async function assertNoApproveDenyControls(page: Page) {
  await expect(page.getByRole('button', { name: /^approve$/i })).toHaveCount(0)
  await expect(page.getByRole('button', { name: /^deny$/i })).toHaveCount(0)
}

export async function assertMarketingHome(page: Page) {
  await page.goto('/')
  await expect(page.locator('section[aria-labelledby="hero-title"]')).toBeVisible()
  await expect(page.getByTestId('overlay-shell')).toHaveCount(0)
  await expect(page.getByTestId('dashboard-shell')).toHaveCount(0)
}

export {
  assertNoDocumentHorizontalOverflow,
  assertNoSeriousAxeViolations,
  trackCrossOriginRequests,
}
