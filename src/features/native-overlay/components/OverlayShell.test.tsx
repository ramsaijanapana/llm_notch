import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createOverlayProps, createSession, createSnapshot } from '../model/testFixtures'
import { OverlayShell } from './OverlayShell'

describe('OverlayShell', () => {
  afterEach(() => {
    cleanup()
  })

  it('renders compact mode with native context attributes', () => {
    render(<OverlayShell {...createOverlayProps({ mode: 'compact', renderContext: 'native' })} />)

    const shell = screen.getByTestId('overlay-shell')
    expect(shell).toHaveAttribute('data-mode', 'compact')
    expect(shell).toHaveAttribute('data-render-context', 'native')
    expect(shell).toHaveAttribute('data-platform', 'macos')
    expect(screen.getByTestId('compact-island')).toBeInTheDocument()
    expect(screen.queryByTestId('peek-panel')).not.toBeInTheDocument()
  })

  it('renders peek mode and exposes preview badge in preview context', () => {
    render(<OverlayShell {...createOverlayProps({ mode: 'peek', renderContext: 'preview' })} />)

    expect(screen.getByTestId('peek-panel')).toBeInTheDocument()
    expect(screen.getByTestId('preview-badge')).toHaveTextContent('Preview')
    expect(screen.queryByTestId('compact-island')).not.toBeInTheDocument()
  })

  it('marks reduced motion on the shell root', () => {
    render(<OverlayShell {...createOverlayProps({ reducedMotion: true })} />)
    expect(screen.getByTestId('overlay-shell')).toHaveAttribute('data-reduced-motion', 'true')
  })

  it('shows warming-up banner in peek mode', () => {
    render(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          connectionState: 'warmingUp',
          snapshot: createSnapshot({ sessions: [] }),
        })}
      />,
    )

    expect(screen.getByTestId('connection-banner')).toHaveTextContent(/Metrics warming up/)
  })

  it('shows custom IPC error message when provided', () => {
    render(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          connectionState: 'ipcError',
          errorMessage: 'Stream channel closed',
        })}
      />,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('Stream channel closed')
  })

  it('forwards dashboard and acknowledge callbacks from peek mode', async () => {
    const user = userEvent.setup()
    const onOpenDashboard = vi.fn()
    const onAcknowledge = vi.fn()

    render(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          onOpenDashboard,
          onAcknowledge,
        })}
      />,
    )

    await user.click(screen.getByRole('button', { name: /open dashboard/i }))
    expect(onOpenDashboard).toHaveBeenCalledTimes(1)

    await user.click(screen.getByRole('button', { name: /acknowledge write tests/i }))
    expect(onAcknowledge).toHaveBeenCalledWith('session-2')
  })
})

describe('CompactIsland via OverlayShell', () => {
  afterEach(() => {
    cleanup()
  })

  it('shows combined CPU and sparkline without token or cost fields', () => {
    render(<OverlayShell {...createOverlayProps()} />)

    expect(screen.getByTestId('compact-cpu')).toHaveTextContent('88%')
    expect(screen.getByTestId('cpu-sparkline')).toBeInTheDocument()
    expect(screen.queryByText(/\$/)).not.toBeInTheDocument()
    expect(screen.queryByText(/token/i)).not.toBeInTheDocument()
  })

  it('renders up to six session dots and an overflow badge', () => {
    const sessions = Array.from({ length: 8 }, (_, index) =>
      createSession({ id: `session-${index}`, label: `Session ${index}` }),
    )

    render(
      <OverlayShell
        {...createOverlayProps({
          snapshot: createSnapshot({ sessions }),
        })}
      />,
    )

    expect(screen.getAllByTestId(/session-dot-/)).toHaveLength(6)
    expect(screen.getByTestId('session-overflow')).toHaveTextContent('+2')
  })
})

describe('PeekPanel via OverlayShell', () => {
  afterEach(() => {
    cleanup()
  })

  it('lists attention items before session rows and omits vendor decision controls', () => {
    render(<OverlayShell {...createOverlayProps({ mode: 'peek' })} />)

    expect(screen.getByTestId('attention-section')).toBeInTheDocument()
    expect(screen.getByTestId('session-row-session-1')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /approve/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deny/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /submit answer/i })).not.toBeInTheDocument()
  })

  it('shows footer combined metrics and All I/O quality label', () => {
    render(<OverlayShell {...createOverlayProps({ mode: 'peek' })} />)

    expect(screen.getByTestId('footer-cpu')).toHaveTextContent('CPU 88%')
    expect(screen.getByTestId('footer-rss')).toHaveTextContent('RSS')
    expect(screen.getByTestId('footer-read')).toHaveTextContent('Read')
    expect(screen.getByTestId('footer-write')).toHaveTextContent('Write')
    expect(screen.getByTestId('footer-processes')).toHaveTextContent('Processes 5')
    expect(screen.getByTestId('quality-note')).toHaveTextContent('Attribution Shared')
    expect(screen.getByTestId('quality-note')).toHaveTextContent('I/O All I/O')
  })

  it('shows empty-state banner when there are no sessions', () => {
    render(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          connectionState: 'empty',
          snapshot: createSnapshot({ sessions: [] }),
        })}
      />,
    )

    expect(screen.getByTestId('connection-banner')).toHaveTextContent(/No active agent sessions/)
  })

  it('uses platform shape attributes on peek panel', () => {
    render(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          platform: 'windows',
        })}
      />,
    )

    expect(screen.getByTestId('peek-panel')).toHaveAttribute('data-platform', 'windows')
  })

  it('updates per-session metrics when snapshot changes', () => {
    const session = createSession()
    const latestMetric = session.latestMetric
    if (!latestMetric) throw new Error('fixture metric missing')
    const { rerender } = render(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          snapshot: createSnapshot({ sessions: [session] }),
        })}
      />,
    )
    const row = screen.getByTestId('session-row-session-1')
    expect(row).toHaveTextContent('42%')

    rerender(
      <OverlayShell
        {...createOverlayProps({
          mode: 'peek',
          snapshot: createSnapshot({
            sessions: [
              {
                ...session,
                latestMetric: { ...latestMetric, cpuCorePercent: 73 },
              },
            ],
          }),
        })}
      />,
    )
    expect(screen.getByTestId('session-row-session-1')).toHaveTextContent('73%')
  })
})
