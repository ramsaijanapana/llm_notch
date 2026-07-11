import { act, renderHook } from '@testing-library/react'
import type { Dispatch } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SimulationAction } from './simulation.types.ts'
import { useSimulationClock } from './useSimulationClock.ts'

describe('useSimulationClock', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('dispatches TICK every second while playing', () => {
    const dispatch = vi.fn<Dispatch<SimulationAction>>()

    renderHook(({ playing }) => useSimulationClock(playing, true, dispatch), {
      initialProps: { playing: true },
    })

    act(() => {
      vi.advanceTimersByTime(2500)
    })

    expect(dispatch).toHaveBeenCalledTimes(2)
    expect(dispatch).toHaveBeenCalledWith({ type: 'TICK' })
  })

  it('does not dispatch ticks while paused', () => {
    const dispatch = vi.fn<Dispatch<SimulationAction>>()

    renderHook(() => useSimulationClock(false, true, dispatch))

    act(() => {
      vi.advanceTimersByTime(5000)
    })

    expect(dispatch).not.toHaveBeenCalled()
  })

  it('does not dispatch ticks when no sessions are running', () => {
    const dispatch = vi.fn<Dispatch<SimulationAction>>()

    renderHook(() => useSimulationClock(true, false, dispatch))

    act(() => {
      vi.advanceTimersByTime(5000)
    })

    expect(dispatch).not.toHaveBeenCalled()
  })

  it('pauses ticking while the document is hidden', () => {
    const dispatch = vi.fn<Dispatch<SimulationAction>>()
    Object.defineProperty(document, 'hidden', {
      configurable: true,
      value: false,
    })

    renderHook(() => useSimulationClock(true, true, dispatch))

    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(dispatch).toHaveBeenCalledTimes(1)

    act(() => {
      Object.defineProperty(document, 'hidden', {
        configurable: true,
        value: true,
      })
      document.dispatchEvent(new Event('visibilitychange'))
      vi.advanceTimersByTime(3000)
    })
    expect(dispatch).toHaveBeenCalledTimes(1)

    act(() => {
      Object.defineProperty(document, 'hidden', {
        configurable: true,
        value: false,
      })
      document.dispatchEvent(new Event('visibilitychange'))
      vi.advanceTimersByTime(2000)
    })
    expect(dispatch).toHaveBeenCalledTimes(3)
  })

  it('cleans up the interval on unmount', () => {
    const dispatch = vi.fn<Dispatch<SimulationAction>>()
    const { unmount } = renderHook(() => useSimulationClock(true, true, dispatch))

    unmount()

    act(() => {
      vi.advanceTimersByTime(3000)
    })

    expect(dispatch).not.toHaveBeenCalled()
  })
})
