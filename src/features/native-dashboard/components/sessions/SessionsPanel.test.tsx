import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mockAdapters, mockEvents, mockSessions } from '../../fixtures/testFixtures'
import { SessionsPanel } from './SessionsPanel'

describe('SessionsPanel', () => {
  afterEach(() => cleanup())

  it('renders attention queue and session lists', () => {
    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
      />,
    )

    expect(screen.getByLabelText('Attention queue')).toBeInTheDocument()
    expect(screen.getByLabelText('Active sessions')).toBeInTheDocument()
    expect(screen.getByLabelText('Recent sessions')).toBeInTheDocument()
  })

  it('shows notify-only guidance and open context action', () => {
    const onOpenContext = vi.fn()

    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
        onOpenContext={onOpenContext}
      />,
    )

    expect(screen.getByText(/resolve in cursor/i)).toBeInTheDocument()
    const openContextButton = screen.getByRole('button', { name: /open dashboard context/i })
    fireEvent.click(openContextButton)
    expect(onOpenContext).toHaveBeenCalledWith('sess-cursor-1')
  })

  it('filters event stream to selected session', () => {
    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
      />,
    )

    expect(screen.getByText(/approval required/i)).toBeInTheDocument()
  })

  it('selects sessions from lists', async () => {
    const user = userEvent.setup()
    const onSelectSession = vi.fn()

    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={onSelectSession}
      />,
    )

    const activeList = screen.getByLabelText('Active sessions')
    await user.click(within(activeList).getByRole('button', { name: /write integration tests/i }))
    expect(onSelectSession).toHaveBeenCalledWith('sess-claude-1')
  })

  it('acknowledges attention queue items when wired', async () => {
    const user = userEvent.setup()
    const onAcknowledge = vi.fn()

    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
        onAcknowledge={onAcknowledge}
      />,
    )

    await user.click(screen.getByRole('button', { name: /acknowledge refactor auth middleware/i }))
    expect(onAcknowledge).toHaveBeenCalledWith('sess-cursor-1')
    expect(
      within(screen.getByLabelText('Attention queue')).getByText(/approval needed/i),
    ).toBeInTheDocument()
  })

  it('shows metric strip with quality labels', () => {
    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
      />,
    )

    expect(screen.getByRole('group', { name: /current session metrics/i })).toBeInTheDocument()
    expect(screen.getByLabelText('Metric quality labels')).toBeInTheDocument()
  })

  it('renders empty state', () => {
    render(
      <SessionsPanel
        sessions={[]}
        events={[]}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
        loadState="empty"
      />,
    )

    expect(screen.getByText(/no sessions/i)).toBeInTheDocument()
  })

  it('renders event log for selected session', () => {
    render(
      <SessionsPanel
        sessions={mockSessions}
        selectedSessionId="sess-cursor-1"
        events={mockEvents}
        adapters={mockAdapters}
        onSelectSession={vi.fn()}
      />,
    )

    expect(screen.getByRole('log')).toBeInTheDocument()
  })
})
