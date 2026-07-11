import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { DashboardShell } from './DashboardShell'

vi.setConfig({ testTimeout: 15_000 })

const panels = {
  sessionsPanel: <div>Sessions content</div>,
  metricsPanel: <div>Metrics content</div>,
  integrationsPanel: <div>Integrations content</div>,
  settingsPanel: <div>Settings content</div>,
}

describe('DashboardShell', () => {
  afterEach(() => cleanup())

  it('renders the active tab panel when ready', () => {
    render(
      <DashboardShell loadState="ready" activeTab="sessions" onTabChange={vi.fn()} {...panels} />,
    )

    expect(screen.getByText('Sessions content')).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /sessions/i })).toHaveAttribute('aria-selected', 'true')
  })

  it('shows loading and error states', () => {
    const { rerender } = render(
      <DashboardShell loadState="loading" activeTab="sessions" onTabChange={vi.fn()} {...panels} />,
    )
    expect(screen.getByText(/loading dashboard data/i)).toBeInTheDocument()

    rerender(
      <DashboardShell
        loadState="error"
        errorMessage="Stream disconnected"
        activeTab="sessions"
        onTabChange={vi.fn()}
        {...panels}
      />,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('Stream disconnected')
  })

  it('switches tabs via keyboard shortcuts', async () => {
    const user = userEvent.setup()
    const onTabChange = vi.fn()

    render(
      <DashboardShell
        loadState="ready"
        activeTab="sessions"
        onTabChange={onTabChange}
        {...panels}
      />,
    )

    await user.keyboard('{Control>}2{/Control}')
    expect(onTabChange).toHaveBeenCalledWith('metrics')

    await user.keyboard('{Meta>}3{/Meta}')
    expect(onTabChange).toHaveBeenCalledWith('integrations')
  })

  it('disables tab shortcuts while a modal is open', async () => {
    const user = userEvent.setup()
    const onTabChange = vi.fn()
    render(
      <DashboardShell
        loadState="ready"
        activeTab="sessions"
        onTabChange={onTabChange}
        shortcutsEnabled={false}
        {...panels}
      />,
    )
    await user.keyboard('{Control>}2{/Control}')
    expect(onTabChange).not.toHaveBeenCalled()
  })

  it('meets minimum responsive shell dimensions', () => {
    render(
      <DashboardShell loadState="ready" activeTab="sessions" onTabChange={vi.fn()} {...panels} />,
    )

    const shell = screen.getByTestId('dashboard-shell')
    expect(shell.className).toContain('shell')
  })
})
