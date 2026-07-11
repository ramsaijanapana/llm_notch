import type { Dispatch } from 'react'
import { useEffect, useRef } from 'react'
import type { SimulationAction } from './simulation.types.ts'

export function useSimulationClock(
  playing: boolean,
  hasRunningSession: boolean,
  dispatch: Dispatch<SimulationAction>,
): void {
  const dispatchRef = useRef(dispatch)
  dispatchRef.current = dispatch

  useEffect(() => {
    if (!playing || !hasRunningSession) {
      return
    }

    let intervalId: ReturnType<typeof setInterval> | undefined

    const startInterval = () => {
      if (intervalId !== undefined || document.hidden) {
        return
      }

      intervalId = setInterval(() => {
        dispatchRef.current({ type: 'TICK' })
      }, 1000)
    }

    const stopInterval = () => {
      if (intervalId === undefined) {
        return
      }

      clearInterval(intervalId)
      intervalId = undefined
    }

    const handleVisibilityChange = () => {
      if (document.hidden) {
        stopInterval()
        return
      }

      startInterval()
    }

    startInterval()
    document.addEventListener('visibilitychange', handleVisibilityChange)

    return () => {
      stopInterval()
      document.removeEventListener('visibilitychange', handleVisibilityChange)
    }
  }, [playing, hasRunningSession])
}
