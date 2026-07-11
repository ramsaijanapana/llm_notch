import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SessionTabs } from './SessionTabs'
import { createMockDispatch, createMockState } from './testFixtures'

const mockUseSimulation = vi.fn()

vi.mock('../model/SimulationProvider', () => ({
  useSimulation: () => mockUseSimulation(),
}))

describe('SessionTabs', () => {
  const { dispatch, calls } = createMockDispatch()

  afterEach(() => {
    cleanup()
  })

  beforeEach(() => {
    calls.length = 0
    mockUseSimulation.mockReturnValue({
      state: createMockState(),
      dispatch,
    })
  })

  it('renders a tab for each session with the selected tab focused in the tab order', () => {
    render(<SessionTabs sessions={createMockState().sessions} selectedId="builder" />)

    const tabs = screen.getAllByRole('tab')
    expect(tabs).toHaveLength(4)
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true')
    expect(tabs[0]).toHaveAttribute('tabindex', '0')
    expect(tabs[1]).toHaveAttribute('tabindex', '-1')
  })

  it('selects a session when a tab is clicked', async () => {
    const user = userEvent.setup()
    render(<SessionTabs sessions={createMockState().sessions} selectedId="builder" />)

    await user.click(screen.getByRole('tab', { name: /tester/i }))

    expect(calls).toContainEqual({
      type: 'SELECT_SESSION',
      sessionId: 'tester',
    })
  })

  it('moves selection with ArrowRight and ArrowLeft', async () => {
    const user = userEvent.setup()
    render(<SessionTabs sessions={createMockState().sessions} selectedId="builder" />)

    const builderTab = screen.getByRole('tab', { name: /builder/i })
    builderTab.focus()

    await user.keyboard('{ArrowRight}')
    expect(calls).toContainEqual({
      type: 'SELECT_SESSION',
      sessionId: 'tester',
    })

    await user.keyboard('{ArrowLeft}')
    expect(calls).toContainEqual({
      type: 'SELECT_SESSION',
      sessionId: 'builder',
    })
  })

  it('wraps tab selection at the ends', async () => {
    const user = userEvent.setup()
    render(<SessionTabs sessions={createMockState().sessions} selectedId="builder" />)

    const builderTab = screen.getByRole('tab', { name: /builder/i })
    builderTab.focus()

    await user.keyboard('{ArrowLeft}')
    expect(calls).toContainEqual({
      type: 'SELECT_SESSION',
      sessionId: 'writer',
    })
  })

  it('references persistent tabpanels for every session tab', () => {
    render(
      <>
        <SessionTabs sessions={createMockState().sessions} selectedId="builder" />
        {createMockState().sessions.map((session) => (
          <div
            key={session.id}
            role="tabpanel"
            id={`session-panel-${session.id}`}
            aria-labelledby={`session-tab-${session.id}`}
            hidden={session.id !== 'builder'}
          />
        ))}
      </>,
    )

    const tabs = screen.getAllByRole('tab')
    for (const tab of tabs) {
      const panelId = tab.getAttribute('aria-controls')
      expect(panelId).toBeTruthy()
      if (!panelId) {
        throw new Error('Tab is missing aria-controls')
      }
      expect(document.getElementById(panelId)).toBeInTheDocument()
    }
  })

  it('jumps to first and last tabs with Home and End', async () => {
    const user = userEvent.setup()
    render(<SessionTabs sessions={createMockState().sessions} selectedId="tester" />)

    const testerTab = screen.getByRole('tab', { name: /tester/i })
    testerTab.focus()

    await user.keyboard('{Home}')
    expect(calls).toContainEqual({
      type: 'SELECT_SESSION',
      sessionId: 'builder',
    })

    await user.keyboard('{End}')
    expect(calls).toContainEqual({
      type: 'SELECT_SESSION',
      sessionId: 'writer',
    })
  })
})
