import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  buildHistory,
  mockAgentMetrics,
  mockAggregate,
  mockHost,
} from '../../fixtures/testFixtures'
import { MetricsPanel } from './MetricsPanel'

describe('MetricsPanel', () => {
  afterEach(() => cleanup())

  it('renders host and aggregate cards with charts', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
      />,
    )

    expect(screen.getByText('Host')).toBeInTheDocument()
    expect(screen.getByText('Aggregate agents')).toBeInTheDocument()
    expect(screen.getAllByRole('img').length).toBeGreaterThan(0)
  })

  it('changes history range via controls', async () => {
    const user = userEvent.setup()
    const onHistoryRangeChange = vi.fn()

    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={onHistoryRangeChange}
      />,
    )

    await user.click(screen.getByRole('button', { name: /24 hours/i }))
    expect(onHistoryRangeChange).toHaveBeenCalledWith('24h')
  })

  it('shows warming and caveat messaging', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="1h"
        onHistoryRangeChange={vi.fn()}
        warmingUp
      />,
    )

    expect(screen.getByText(/warming up/i)).toBeInTheDocument()
    expect(screen.getByText(/rss reflects attributed resident memory/i)).toBeInTheDocument()
    expect(screen.getByText(/gpu utilization — unsupported/i)).toBeInTheDocument()
    expect(screen.getByText(/network throughput — unsupported/i)).toBeInTheDocument()
  })

  it('renders per-agent table', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
      />,
    )

    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(screen.getByText('cursor')).toBeInTheDocument()
  })

  it('shows empty state when metrics missing', () => {
    render(
      <MetricsPanel
        agents={{}}
        history={buildHistory(0)}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
        loadState="empty"
      />,
    )

    expect(screen.getByText(/metrics unavailable/i)).toBeInTheDocument()
  })

  it('shows persisted history loading, error, and empty states', () => {
    const common = {
      host: mockHost,
      aggregate: mockAggregate,
      agents: mockAgentMetrics,
      history: buildHistory(0),
      historyRange: '24h' as const,
      onHistoryRangeChange: vi.fn(),
    }
    const { rerender } = render(<MetricsPanel {...common} historyLoadState="loading" />)
    expect(screen.getByText(/loading persisted history/i)).toBeInTheDocument()

    rerender(
      <MetricsPanel
        {...common}
        historyLoadState="error"
        historyError="History database unavailable"
      />,
    )
    expect(screen.getByText(/history database unavailable/i)).toBeInTheDocument()

    rerender(<MetricsPanel {...common} historyLoadState="empty" />)
    expect(screen.getByText(/no history in this range/i)).toBeInTheDocument()
  })

  it('reports partial coverage and downsampling without claiming a full range', () => {
    const history = buildHistory()
    history.hostCoverage = {
      ...history.hostCoverage,
      actualFirstMs: history.requestedEndMs - 58 * 60_000,
      actualLastMs: history.requestedEndMs,
      totalPoints: 20_001,
      returnedPoints: 720,
      downsampled: true,
    }
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={history}
        historyRange="24h"
        onHistoryRangeChange={vi.fn()}
      />,
    )
    expect(screen.getByTestId('host-history-coverage')).toHaveTextContent(
      /58m of selected 24 hours/i,
    )
    expect(screen.getByTestId('host-history-coverage')).toHaveTextContent(
      /downsampled 20001 to 720/i,
    )
  })

  it('disables ranges impossible under retention', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        disabledHistoryRanges={['24h']}
        onHistoryRangeChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: /24 hours/i })).toBeDisabled()
  })

  it('surfaces credential setup hints for gated quota providers', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
        quotas={[
          {
            service: 'claude',
            displayName: 'Claude',
            availability: 'unavailable',
            authentication: 'required',
            message: 'set ANTHROPIC_API_KEY to enable quota probes',
          },
        ]}
      />,
    )

    expect(screen.getByText(/set ANTHROPIC_API_KEY to enable quota probes/i)).toBeInTheDocument()
  })

  it('refreshes quotas via the refresh button', async () => {
    const user = userEvent.setup()
    const onRefreshQuotas = vi.fn()

    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
        onRefreshQuotas={onRefreshQuotas}
      />,
    )

    await user.click(screen.getByRole('button', { name: /refresh quotas/i }))
    expect(onRefreshQuotas).toHaveBeenCalledTimes(1)
  })

  it('disables refresh while quota refresh is loading', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
        onRefreshQuotas={vi.fn()}
        quotaRefreshState="loading"
      />,
    )

    const button = screen.getByRole('button', { name: /refreshing/i })
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('aria-busy', 'true')
  })

  it('shows fresh/stale indicator when quota snapshots include timestamps', () => {
    const nowMs = 1_700_000_000_000
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
        nowMs={nowMs}
        quotas={[
          {
            service: 'claude',
            displayName: 'Claude',
            availability: 'available',
            remaining: 42,
            unit: 'requests',
            observedAtMs: nowMs - 2 * 60_000,
            freshness: 'fresh',
          },
        ]}
      />,
    )

    expect(screen.getByTestId('quota-observed-status')).toHaveTextContent(/updated 2m ago/i)
    expect(screen.getByText('Fresh')).toBeInTheDocument()
  })

  it('does not show freshness indicator without observed timestamps', () => {
    render(
      <MetricsPanel
        host={mockHost}
        aggregate={mockAggregate}
        agents={mockAgentMetrics}
        history={buildHistory()}
        historyRange="15m"
        onHistoryRangeChange={vi.fn()}
        quotas={[
          {
            service: 'claude',
            displayName: 'Claude',
            availability: 'unavailable',
            authentication: 'required',
            message: 'set ANTHROPIC_API_KEY to enable quota probes',
          },
        ]}
      />,
    )

    expect(screen.queryByTestId('quota-observed-status')).not.toBeInTheDocument()
    expect(screen.getByText(/set ANTHROPIC_API_KEY to enable quota probes/i)).toBeInTheDocument()
  })
})
