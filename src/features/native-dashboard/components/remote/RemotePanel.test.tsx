import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  FIXED_NOW_MS,
  mockRemoteBackendStatus,
  mockRemoteHosts,
} from '../../fixtures/testFixtures'
import type { AgentSession } from '../../../../native/contracts'
import { RemotePanel } from './RemotePanel'

const now = FIXED_NOW_MS

function remoteSession(
  overrides: Partial<AgentSession> & Pick<AgentSession, 'id'>,
): AgentSession {
  return {
    source: 'cursor',
    externalSessionId: overrides.id,
    label: 'Remote task',
    workspaceLabel: 'remote:dev-box',
    status: 'running',
    attention: 'none',
    startedAtMs: now - 600_000,
    lastEventAtMs: now - 60_000,
    ...overrides,
  }
}

describe('RemotePanel', () => {
  afterEach(() => cleanup())

  it('shows honest backend unavailability and empty host state', () => {
    render(
      <RemotePanel
        hosts={[]}
        backendStatus={mockRemoteBackendStatus}
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByRole('heading', { name: /ssh relay backend/i })).toBeInTheDocument()
    expect(screen.getByText(/not available in this build/i)).toBeInTheDocument()
    expect(screen.getByText(/no remote hosts configured/i)).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /add ssh host/i })).toBeInTheDocument()
  })

  it('disables lifecycle actions when backend is unavailable', () => {
    render(
      <RemotePanel
        hosts={mockRemoteHosts}
        backendStatus={mockRemoteBackendStatus}
        lifecycleActionsAvailable={false}
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByLabelText('dev-box remote host')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /preview deploy plan/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /start relay/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /stop relay/i })).toBeDisabled()
  })

  it('invokes lifecycle callbacks when actions are available', async () => {
    const user = userEvent.setup()
    const onPlanDeploy = vi.fn()
    const onStartRelay = vi.fn()
    const onStopRelay = vi.fn()

    render(
      <RemotePanel
        hosts={mockRemoteHosts}
        backendStatus={{ availability: 'available' }}
        lifecycleActionsAvailable
        onPlanDeploy={onPlanDeploy}
        onStartRelay={onStartRelay}
        onStopRelay={onStopRelay}
        onDismissPlan={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    await user.click(screen.getByRole('button', { name: /preview deploy plan/i }))
    expect(onPlanDeploy).toHaveBeenCalledWith('dev-box')

    await user.click(screen.getByRole('button', { name: /start relay/i }))
    expect(onStartRelay).toHaveBeenCalledWith('dev-box')

    await user.click(screen.getByRole('button', { name: /stop relay/i }))
    expect(onStopRelay).toHaveBeenCalledWith('dev-box')
  })

  it('invokes host config callbacks when configuration actions are available', async () => {
    const user = userEvent.setup()
    const onAddHost = vi.fn()
    const onRemoveHost = vi.fn()

    render(
      <RemotePanel
        hosts={mockRemoteHosts}
        backendStatus={mockRemoteBackendStatus}
        hostConfigActionsAvailable
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        onAddHost={onAddHost}
        onRemoveHost={onRemoveHost}
        nowMs={FIXED_NOW_MS}
      />,
    )

    await user.type(screen.getByLabelText(/host id/i), 'lab-box')
    await user.type(screen.getByLabelText(/destination/i), 'dev@lab.internal')
    await user.click(screen.getByRole('button', { name: /save host/i }))
    expect(onAddHost).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'lab-box',
        destination: 'dev@lab.internal',
        hostKeyPolicy: 'strict',
        connectTimeoutSeconds: 10,
      }),
    )

    await user.click(screen.getByRole('button', { name: /remove host/i }))
    expect(onRemoveHost).toHaveBeenCalledWith('dev-box')
  })

  it('renders deployment plan preview and dismisses it', async () => {
    const user = userEvent.setup()
    const onDismissPlan = vi.fn()
    const onExecuteDeploy = vi.fn()

    render(
      <RemotePanel
        hosts={mockRemoteHosts}
        backendStatus={{ availability: 'available' }}
        lifecycleActionsAvailable
        pendingDeployPlan={{
          hostId: 'dev-box',
          availability: 'available',
          steps: [
            { type: 'probeTarget' },
            { type: 'createPrivateDirectory', remoteDirectory: '~/.llm-notch/bin' },
          ],
        }}
        onPlanDeploy={vi.fn()}
        onExecuteDeploy={onExecuteDeploy}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={onDismissPlan}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByRole('heading', { name: /deployment plan preview/i })).toBeInTheDocument()
    expect(screen.getByText(/probe remote target/i)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /execute deploy/i }))
    expect(onExecuteDeploy).toHaveBeenCalledWith('dev-box')
    await user.click(screen.getByRole('button', { name: /close preview/i }))
    expect(onDismissPlan).toHaveBeenCalled()
  })

  it('reflects live connection badge updates from host props', () => {
    const { rerender } = render(
      <RemotePanel
        hosts={mockRemoteHosts}
        backendStatus={{ availability: 'available' }}
        lifecycleActionsAvailable
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByText(/^Disconnected$/)).toBeInTheDocument()

    rerender(
      <RemotePanel
        hosts={[
          {
            ...mockRemoteHosts[0]!,
            availability: 'available',
            connectionState: 'streaming',
            message: null,
          },
        ]}
        backendStatus={{ availability: 'available' }}
        lifecycleActionsAvailable
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByText(/^Streaming$/)).toBeInTheDocument()
  })

  it('shows honest ingest stats derived from remote-attributed sessions', () => {
    render(
      <RemotePanel
        hosts={mockRemoteHosts}
        sessions={[
          remoteSession({ id: 'remote-active', status: 'running', lastEventAtMs: now - 30_000 }),
          remoteSession({
            id: 'remote-waiting',
            status: 'waiting',
            lastEventAtMs: now - 120_000,
          }),
          remoteSession({
            id: 'remote-done',
            status: 'completed',
            lastEventAtMs: now - 3_600_000,
            endedAtMs: now - 3_600_000,
          }),
        ]}
        backendStatus={{ availability: 'available' }}
        lifecycleActionsAvailable
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        nowMs={now}
      />,
    )

    const hostCard = screen.getByLabelText('dev-box remote host')

    expect(within(hostCard).getByText('Ingested sessions')).toBeInTheDocument()
    expect(within(hostCard).getByText('3')).toBeInTheDocument()
    expect(within(hostCard).getByText('Active ingested')).toBeInTheDocument()
    expect(within(hostCard).getByText('2')).toBeInTheDocument()
    expect(within(hostCard).getByText('Last ingested event')).toBeInTheDocument()
    expect(within(hostCard).getByText('just now')).toBeInTheDocument()
  })

  it('shows zero ingest stats when no remote-attributed sessions exist', () => {
    render(
      <RemotePanel
        hosts={mockRemoteHosts}
        sessions={[]}
        backendStatus={{ availability: 'available' }}
        lifecycleActionsAvailable
        onPlanDeploy={vi.fn()}
        onStartRelay={vi.fn()}
        onStopRelay={vi.fn()}
        onDismissPlan={vi.fn()}
        nowMs={now}
      />,
    )

    const hostCard = screen.getByLabelText('dev-box remote host')

    expect(within(hostCard).getByText('Ingested sessions')).toBeInTheDocument()
    expect(within(hostCard).getAllByText('0')).toHaveLength(2)
    expect(within(hostCard).getByText('None')).toBeInTheDocument()
  })
})
