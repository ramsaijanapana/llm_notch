import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TerminalDrawer } from './TerminalDrawer'
import { createMockDispatch, createMockState } from './testFixtures'

const mockUseSimulation = vi.fn()

vi.mock('../model/SimulationProvider', () => ({
  useSimulation: () => mockUseSimulation(),
}))

describe('TerminalDrawer', () => {
  const { dispatch, calls } = createMockDispatch()

  afterEach(() => {
    cleanup()
  })

  beforeEach(() => {
    calls.length = 0
    mockUseSimulation.mockReturnValue({
      state: createMockState({ terminalOpen: true }),
      dispatch,
    })
  })

  it('restores focus to the jump trigger when closed', async () => {
    const user = userEvent.setup()
    const jumpTriggerRef = createRef<HTMLButtonElement>()

    render(
      <>
        <button ref={jumpTriggerRef} type="button">
          Jump to workspace
        </button>
        <TerminalDrawer jumpTriggerRef={jumpTriggerRef} />
      </>,
    )

    jumpTriggerRef.current?.focus()
    expect(document.activeElement).toBe(jumpTriggerRef.current)

    await user.tab()
    await user.click(screen.getByRole('button', { name: /close simulated terminal/i }))

    expect(calls).toContainEqual({ type: 'CLOSE_TERMINAL' })
    expect(document.activeElement).toBe(jumpTriggerRef.current)
  })
})
