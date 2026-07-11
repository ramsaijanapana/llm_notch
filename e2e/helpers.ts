import AxeBuilder from '@axe-core/playwright'
import { expect, type Page, type Request } from '@playwright/test'

const APP_ORIGIN = new URL(process.env.PLAYWRIGHT_BASE_URL ?? 'http://127.0.0.1:4173').origin

export async function gotoHome(page: Page, options?: { reducedMotion?: boolean }) {
  if (options?.reducedMotion) {
    await page.emulateMedia({ reducedMotion: 'reduce' })
  } else {
    await page.emulateMedia({ reducedMotion: 'no-preference' })
  }

  await page.goto('/')
  await page.waitForLoadState('domcontentloaded')
}

export function heroDemoLink(page: Page) {
  return page.locator('section[aria-labelledby="hero-title"]').getByRole('link', {
    name: 'Play the demo',
  })
}

export function consoleCapsule(page: Page) {
  return page.getByRole('button', { name: /interactive demo console/i })
}

export async function expandDemoViaHero(page: Page) {
  await heroDemoLink(page).click()
  await expect(consoleCapsule(page)).toHaveAttribute('aria-expanded', 'true')
}

export async function ensureDemoExpanded(page: Page) {
  const capsule = consoleCapsule(page)
  if ((await capsule.getAttribute('aria-expanded')) !== 'true') {
    await expandDemoViaHero(page)
  }
}

export function demoStatus(page: Page) {
  return page.locator('#demo [role="status"]')
}

export async function resetSimulation(page: Page, options?: { keepCollapsed?: boolean }) {
  await ensureDemoExpanded(page)
  await page.getByRole('button', { name: /reset simulation/i }).click()
  await expect(demoStatus(page)).toContainText(/simulation reset/i)
  if (!options?.keepCollapsed) {
    await ensureDemoExpanded(page)
  }
}

export async function selectSessionTab(page: Page, name: RegExp | string) {
  await page.getByRole('tab', { name }).click()
}

export async function expectPlaybackPlaying(page: Page) {
  await expect(page.getByRole('button', { name: 'Pause' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Pause' })).toHaveAttribute('aria-pressed', 'true')
}

export async function expectPlaybackPaused(page: Page) {
  const capsule = consoleCapsule(page)

  if ((await capsule.getAttribute('aria-expanded')) === 'true') {
    await expect(page.getByRole('button', { name: 'Play' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Play' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
  }

  await expect(capsule).toHaveAttribute('aria-label', /paused/i)
}

export async function waitForSimulationTick(page: Page, initialProgress = 38) {
  const progressBar = page.getByRole('progressbar', { name: 'Task progress' })
  await expect
    .poll(async () => Number(await progressBar.getAttribute('aria-valuenow')), {
      timeout: 4_000,
      message: 'Expected builder progress to advance after a simulation tick',
    })
    .toBeGreaterThan(initialProgress)
}

export async function assertNoDocumentHorizontalOverflow(page: Page) {
  const hasOverflow = await page.evaluate(() => {
    const root = document.documentElement
    return root.scrollWidth > root.clientWidth + 1
  })
  expect(hasOverflow, 'Document should not scroll horizontally').toBe(false)
}

export function trackCrossOriginRequests(page: Page) {
  const crossOriginRequests: string[] = []

  const onRequest = (request: Request) => {
    const requestUrl = new URL(request.url())

    if (
      requestUrl.protocol === 'data:' ||
      requestUrl.protocol === 'blob:' ||
      requestUrl.origin === APP_ORIGIN
    ) {
      return
    }

    crossOriginRequests.push(request.url())
  }

  page.on('request', onRequest)

  return {
    getCrossOriginRequests: () => [...crossOriginRequests],
    stop: () => page.off('request', onRequest),
  }
}

export async function assertNoSeriousAxeViolations(page: Page, context?: string) {
  let builder = new AxeBuilder({ page })

  if (context) {
    builder = builder.include(context)
  }

  const results = await builder.analyze()
  const violations = results.violations.filter(
    (violation) => violation.impact === 'critical' || violation.impact === 'serious',
  )

  expect(
    violations,
    violations.length > 0
      ? `Accessibility violations:\n${violations
          .map((violation) => `${violation.id} (${violation.impact}): ${violation.help}`)
          .join('\n')}`
      : undefined,
  ).toEqual([])
}
