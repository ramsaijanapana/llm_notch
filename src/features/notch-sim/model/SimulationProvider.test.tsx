import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { SimulationProvider, useSimulation } from './SimulationProvider.tsx'

function Probe() {
  const { state, dispatch, prefersReducedMotion } = useSimulation()

  return (
    <div>
      <span data-testid="selected">{state.selectedId}</span>
      <span data-testid="playing">{String(state.playing)}</span>
      <span data-testid="reduced-motion">{String(prefersReducedMotion)}</span>
      <button
        type="button"
        onClick={() => dispatch({ type: 'SELECT_SESSION', sessionId: 'tester' })}
      >
        select tester
      </button>
      <button type="button" onClick={() => dispatch({ type: 'RESET' })}>
        reset
      </button>
    </div>
  )
}

describe('SimulationProvider', () => {
  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
  })

  it('shares one simulation instance across consumers', async () => {
    const user = userEvent.setup()

    render(
      <SimulationProvider prefersReducedMotion>
        <Probe />
        <Probe />
      </SimulationProvider>,
    )

    const selectedLabels = screen.getAllByTestId('selected')
    const playingLabels = screen.getAllByTestId('playing')
    const reducedMotionLabels = screen.getAllByTestId('reduced-motion')

    expect(selectedLabels).toHaveLength(2)
    expect(playingLabels).toHaveLength(2)
    expect(playingLabels[0]).toHaveTextContent('false')
    expect(playingLabels[1]).toHaveTextContent('false')
    expect(reducedMotionLabels[0]).toHaveTextContent('true')

    const selectTesterButtons = screen.getAllByRole('button', { name: 'select tester' })
    const firstSelectTesterButton = selectTesterButtons[0]
    expect(firstSelectTesterButton).toBeDefined()
    await user.click(firstSelectTesterButton as HTMLElement)

    expect(screen.getAllByTestId('selected')[0]).toHaveTextContent('tester')
    expect(screen.getAllByTestId('selected')[1]).toHaveTextContent('tester')
  })

  it('starts paused when reduced motion is preferred', () => {
    render(
      <SimulationProvider prefersReducedMotion>
        <Probe />
      </SimulationProvider>,
    )

    expect(screen.getByTestId('playing')).toHaveTextContent('false')
    expect(screen.getByTestId('reduced-motion')).toHaveTextContent('true')
  })

  it('preserves reduced-motion playback preference on reset', async () => {
    const user = userEvent.setup()

    render(
      <SimulationProvider prefersReducedMotion>
        <Probe />
      </SimulationProvider>,
    )

    await user.click(screen.getByRole('button', { name: 'select tester' }))
    await user.click(screen.getByRole('button', { name: 'reset' }))

    expect(screen.getByTestId('selected')).toHaveTextContent('builder')
    expect(screen.getByTestId('playing')).toHaveTextContent('false')
    expect(screen.getByTestId('reduced-motion')).toHaveTextContent('true')
  })

  it('pauses playback when reduced motion preference becomes true at runtime', async () => {
    let changeHandler: ((event: MediaQueryListEvent) => void) | undefined

    vi.spyOn(window, 'matchMedia').mockImplementation(
      (query: string) =>
        ({
          matches: false,
          media: query,
          onchange: null,
          addEventListener: (_type: string, handler: EventListenerOrEventListenerObject) => {
            if (typeof handler === 'function') {
              changeHandler = handler as (event: MediaQueryListEvent) => void
            }
          },
          removeEventListener: () => {},
          addListener: () => {},
          removeListener: () => {},
          dispatchEvent: () => true,
        }) as MediaQueryList,
    )

    render(
      <SimulationProvider>
        <Probe />
      </SimulationProvider>,
    )

    expect(screen.getByTestId('playing')).toHaveTextContent('true')

    await act(async () => {
      changeHandler?.({ matches: true } as MediaQueryListEvent)
    })

    expect(screen.getByTestId('playing')).toHaveTextContent('false')
    expect(screen.getByTestId('reduced-motion')).toHaveTextContent('true')
  })

  it('throws when useSimulation is used outside the provider', () => {
    function Orphan() {
      useSimulation()
      return null
    }

    expect(() => render(<Orphan />)).toThrow(/SimulationProvider/)
  })
})
