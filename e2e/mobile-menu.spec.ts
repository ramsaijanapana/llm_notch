import { expect, test } from '@playwright/test'
import { consoleCapsule, gotoHome } from './helpers'

test.describe('Mobile navigation menu', () => {
  test.use({ viewport: { width: 390, height: 844 } })

  test('discloses at 390px and closes after anchor selection', async ({ page }) => {
    await gotoHome(page)

    const menuToggle = page.getByRole('button', { name: 'Menu' })
    const navigation = page.getByRole('navigation', { name: 'Primary' })

    await expect(menuToggle).toBeVisible()
    await expect(menuToggle).toHaveAttribute('aria-expanded', 'false')
    await expect(menuToggle).toHaveAttribute('aria-controls', 'primary-navigation')

    await menuToggle.click()
    await expect(menuToggle).toHaveAttribute('aria-expanded', 'true')
    await expect(navigation.getByRole('link', { name: 'Workflow' })).toBeVisible()

    await navigation.getByRole('link', { name: 'Workflow' }).click()
    await expect(menuToggle).toHaveAttribute('aria-expanded', 'false')

    await menuToggle.click()
    await navigation.getByRole('link', { name: 'Demo' }).click()
    await expect(menuToggle).toHaveAttribute('aria-expanded', 'false')
    await expect(consoleCapsule(page)).toHaveAttribute('aria-expanded', 'true')
  })
})
