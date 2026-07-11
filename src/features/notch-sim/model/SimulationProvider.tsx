import {
  createContext,
  type Dispatch,
  type ReactNode,
  useContext,
  useEffect,
  useMemo,
  useReducer,
} from 'react'
import { createInitialState } from './scenario.ts'
import { simulationReducer } from './simulation.reducer.ts'
import type { SimulationAction, SimulationState } from './simulation.types.ts'
import { useReducedMotion } from './useReducedMotion.ts'
import { useSimulationClock } from './useSimulationClock.ts'

interface SimulationContextValue {
  state: SimulationState
  dispatch: Dispatch<SimulationAction>
  prefersReducedMotion: boolean
}

const SimulationContext = createContext<SimulationContextValue | null>(null)

interface SimulationProviderProps {
  children: ReactNode
  prefersReducedMotion?: boolean
}

export function SimulationProvider({ children, prefersReducedMotion }: SimulationProviderProps) {
  const detectedReducedMotion = useReducedMotion()
  const initialReducedMotion = prefersReducedMotion ?? detectedReducedMotion

  const [state, dispatch] = useReducer(simulationReducer, initialReducedMotion, createInitialState)

  useEffect(() => {
    if (prefersReducedMotion !== undefined) {
      return
    }

    dispatch({ type: 'SET_REDUCED_MOTION', value: detectedReducedMotion })
  }, [detectedReducedMotion, prefersReducedMotion])

  const hasRunningSession = state.sessions.some((session) => session.phase === 'running')
  useSimulationClock(state.playing, hasRunningSession, dispatch)

  const value = useMemo(
    () => ({
      state,
      dispatch,
      prefersReducedMotion: state.prefersReducedMotion,
    }),
    [state],
  )

  return <SimulationContext.Provider value={value}>{children}</SimulationContext.Provider>
}

export function useSimulation(): SimulationContextValue {
  const context = useContext(SimulationContext)

  if (!context) {
    throw new Error('useSimulation must be used within a SimulationProvider')
  }

  return context
}
