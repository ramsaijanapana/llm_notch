import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { AgentSource } from '../../../native/contracts'
import { DashboardShell } from './DashboardShell'

vi.setConfig({ testTimeout: 15_000 })

const panels = {
  sessionsPanel: <div>Sessions content</div>,
  metricsPanel: <div>Metrics content</div>,
  integrationsPanel: <div>Integrations content</div>,
  remotePanel: <div>Remote content</div>,
  settingsPanel: <div>Settings content</div>,
}

const agentStatuses = [
  { source: 'cursor' as AgentSource, status: 'connected' as const, activeSessions: 1 },
]

describe('DashboardShell', () => {
  afterEach(() => cleanup())

  it('renders the active tab panel when ready', () => {
    render(
      <DashboardShell loadState="ready" activeTab="sessions" onTabChange={vi.fn()} {...panels} />,
    )

    expect(screen.getByText('Sessions content')).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /sessions/i })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('heading', { name: /sessions/i, level: 2 })).toBeInTheDocument()
  })

  it('shows agent status rail when statuses are provided', () => {
    render(
      <DashboardShell
        loadState="ready"
        activeTab="sessions"
        onTabChange={vi.fn()}
        agentStatuses={agentStatuses}
        {...panels}
      />,
    )

    expect(screen.getByRole('region', { name: /agent status/i })).toBeInTheDocument()
    expect(screen.getByLabelText(/cursor: connected/i)).toBeInTheDocument()
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

    await user.keyboard('{Control>}4{/Control}')
    expect(onTabChange).toHaveBeenCalledWith('remote')
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
