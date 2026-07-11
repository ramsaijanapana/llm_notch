import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DecisionPanel } from './DecisionPanel'
import { createMockDispatch, createMockState, mockSessions } from './testFixtures'

const mockUseSimulation = vi.fn()

vi.mock('../model/SimulationProvider', () => ({
  useSimulation: () => mockUseSimulation(),
}))

describe('DecisionPanel', () => {
  const { dispatch, calls } = createMockDispatch()
  const needsAnswerSession = mockSessions.find((session) => session.id === 'reviewer')
  const needsApprovalSession = mockSessions.find((session) => session.id === 'tester')
  const completedSession = mockSessions.find((session) => session.id === 'writer')

  afterEach(() => {
    cleanup()
  })

  beforeEach(() => {
    calls.length = 0
    mockUseSimulation.mockReturnValue({
      state: createMockState({ selectedId: 'reviewer' }),
      dispatch,
    })
  })

  it('shows a visible answer label', () => {
    if (!needsAnswerSession) throw new Error('fixture missing reviewer session')

    render(<DecisionPanel session={needsAnswerSession} />)

    expect(screen.getByLabelText(/your answer/i)).toBeInTheDocument()
  })

  it('blocks blank answer submission and shows validation message', async () => {
    const user = userEvent.setup()
    if (!needsAnswerSession) throw new Error('fixture missing reviewer session')

    render(<DecisionPanel session={needsAnswerSession} />)

    await user.click(screen.getByRole('button', { name: /submit answer/i }))

    expect(screen.getByRole('alert')).toHaveTextContent(/enter an answer/i)
    expect(calls.some((call) => call.type === 'SUBMIT_ANSWER')).toBe(false)
  })

  it('dispatches SUBMIT_ANSWER when answer is provided', async () => {
    const user = userEvent.setup()
    if (!needsAnswerSession) throw new Error('fixture missing reviewer session')

    render(<DecisionPanel session={needsAnswerSession} />)

    await user.type(screen.getByLabelText(/your answer/i), 'RFC 7807')
    await user.click(screen.getByRole('button', { name: /submit answer/i }))

    expect(calls).toContainEqual({
      type: 'SUBMIT_ANSWER',
      answer: 'RFC 7807',
    })
  })

  it('moves focus to the persistent status summary after approve', async () => {
    const user = userEvent.setup()
    if (!needsApprovalSession) throw new Error('fixture missing tester session')

    render(<DecisionPanel session={needsApprovalSession} />)

    await user.click(screen.getByRole('button', { name: /approve/i }))

    expect(document.activeElement).toHaveTextContent(/needs approval/i)
  })

  it('shows the pending approval command for the tester session', () => {
    if (!needsApprovalSession) throw new Error('fixture missing tester session')

    render(<DecisionPanel session={needsApprovalSession} />)

    expect(screen.getByText(/approve running: npm test --coverage/i)).toBeInTheDocument()
  })

  it('disables ask agent for completed sessions', () => {
    if (!completedSession) throw new Error('fixture missing writer session')

    render(<DecisionPanel session={completedSession} />)

    expect(screen.getByRole('button', { name: /ask agent/i })).toBeDisabled()
  })

  it('clears draft answer state when the session phase changes', async () => {
    const user = userEvent.setup()
    if (!needsAnswerSession) throw new Error('fixture missing reviewer session')

    const { rerender } = render(<DecisionPanel session={needsAnswerSession} />)

    await user.type(screen.getByLabelText(/your answer/i), 'draft')
    rerender(<DecisionPanel session={{ ...needsAnswerSession, phase: 'running' }} />)
    rerender(
      <DecisionPanel session={{ ...needsAnswerSession, id: 'builder', phase: 'needsAnswer' }} />,
    )

    expect(screen.getByLabelText(/your answer/i)).toHaveValue('')
  })
})
