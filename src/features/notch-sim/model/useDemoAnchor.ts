import { useCallback } from 'react'
import { useSimulation } from './SimulationProvider.tsx'

export function useDemoAnchorClick(): (event: React.MouseEvent<HTMLAnchorElement>) => void {
  const { dispatch, prefersReducedMotion } = useSimulation()

  return useCallback(
    (event: React.MouseEvent<HTMLAnchorElement>) => {
      if (event.currentTarget.hash !== '#demo') {
        return
      }

      dispatch({ type: 'SET_EXPANDED', expanded: true })

      if (!prefersReducedMotion) {
        dispatch({ type: 'PLAY' })
      }
    },
    [dispatch, prefersReducedMotion],
  )
}

export function activateDemoFromSimulation(
  dispatch: ReturnType<typeof useSimulation>['dispatch'],
  prefersReducedMotion: boolean,
): void {
  dispatch({ type: 'SET_EXPANDED', expanded: true })

  if (!prefersReducedMotion) {
    dispatch({ type: 'PLAY' })
  }
}
