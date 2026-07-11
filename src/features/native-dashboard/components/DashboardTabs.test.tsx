import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { DashboardTabs } from './DashboardTabs'

describe('DashboardTabs', () => {
  afterEach(() => cleanup())

  it('renders four semantic tabs with roving focus', () => {
    render(<DashboardTabs activeTab="sessions" onTabChange={vi.fn()} />)

    const tabs = screen.getAllByRole('tab')
    expect(tabs).toHaveLength(4)
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true')
    expect(tabs[0]).toHaveAttribute('tabindex', '0')
    expect(tabs[1]).toHaveAttribute('tabindex', '-1')
  })

  it('changes tabs on click', async () => {
    const user = userEvent.setup()
    const onTabChange = vi.fn()

    render(<DashboardTabs activeTab="sessions" onTabChange={onTabChange} />)
    await user.click(screen.getByRole('tab', { name: /metrics/i }))

    expect(onTabChange).toHaveBeenCalledWith('metrics')
  })

  it('supports arrow key roving selection', async () => {
    const user = userEvent.setup()
    const onTabChange = vi.fn()

    render(<DashboardTabs activeTab="sessions" onTabChange={onTabChange} />)

    const sessionsTab = screen.getByRole('tab', { name: /sessions/i })
    sessionsTab.focus()
    await user.keyboard('{ArrowRight}')

    expect(onTabChange).toHaveBeenCalledWith('metrics')
  })

  it('links tabs to tabpanels', () => {
    render(
      <>
        <DashboardTabs activeTab="settings" onTabChange={vi.fn()} />
        <div
          role="tabpanel"
          id="dashboard-panel-settings"
          aria-labelledby="dashboard-tab-settings"
        />
      </>,
    )

    const settingsTab = screen.getByRole('tab', { name: /settings/i })
    expect(settingsTab).toHaveAttribute('aria-controls', 'dashboard-panel-settings')
    expect(document.getElementById('dashboard-panel-settings')).toBeInTheDocument()
  })
})
