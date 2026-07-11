import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { SimulationProvider } from '../model/SimulationProvider'
import { NotchDemo } from './NotchDemo'

describe('NotchDemo', () => {
  afterEach(() => {
    cleanup()
  })

  it('restores focus to the collapsed console capsule after reset', async () => {
    const user = userEvent.setup()

    render(
      <SimulationProvider prefersReducedMotion>
        <NotchDemo />
      </SimulationProvider>,
    )

    const capsule = screen.getByRole('button', { name: /interactive demo console/i })

    await user.click(capsule)
    expect(capsule).toHaveAttribute('aria-expanded', 'true')

    await user.click(screen.getByRole('button', { name: /reset simulation/i }))

    expect(capsule).toHaveAttribute('aria-expanded', 'false')
    expect(capsule).toHaveFocus()
    expect(screen.queryByRole('button', { name: /reset simulation/i })).not.toBeInTheDocument()
  })
})
