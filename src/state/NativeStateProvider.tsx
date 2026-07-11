import {
  createContext,
  type Dispatch,
  type ReactNode,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
} from 'react'
import { createNativeClient } from '../native/client.ts'
import { PROTOCOL_VERSION } from '../native/contracts.ts'
import { isNativeClientError } from '../native/errors.ts'
import { evaluateStreamSequence } from '../native/streamProcessor.ts'
import type { NativeClient } from '../native/types.ts'
import { createInitialNativeState, nativeReducer } from './native.reducer.ts'
import type { NativeAction, NativeState } from './native.types.ts'

interface NativeStateContextValue {
  state: NativeState
  dispatch: Dispatch<NativeAction>
  client: NativeClient
  prefersReducedMotion: boolean
}

const NativeStateContext = createContext<NativeStateContextValue | null>(null)

export interface NativeStateProviderProps {
  children: ReactNode
  client?: NativeClient
}

export function NativeStateProvider({
  children,
  client: providedClient,
}: NativeStateProviderProps) {
  const clientRef = useRef<NativeClient>(providedClient ?? createNativeClient())
  const subscriptionRef = useRef<{ unsubscribe: () => Promise<void> } | null>(null)
  const lastSequenceRef = useRef<number | null>(null)
  const resyncingRef = useRef(false)

  const [state, dispatch] = useReducer(nativeReducer, clientRef.current.mode, (mode) =>
    createInitialNativeState(mode),
  )

  useEffect(() => {
    dispatch({ type: 'SET_CLIENT_MODE', mode: clientRef.current.mode })
  }, [])

  useEffect(() => {
    let cancelled = false

    const resync = async () => {
      if (resyncingRef.current || cancelled) {
        return
      }

      resyncingRef.current = true
      dispatch({ type: 'SET_CONNECTION', status: 'resyncing' })

      try {
        const bootstrap = await clientRef.current.bootstrap()
        if (cancelled) {
          return
        }

        lastSequenceRef.current = bootstrap.lastSequence
        dispatch({
          type: 'APPLY_BOOTSTRAP',
          snapshot: bootstrap.snapshot,
          lastSequence: bootstrap.lastSequence,
          events: bootstrap.events,
        })
        dispatch({ type: 'CLEAR_RESYNC' })
      } catch (error) {
        const message = error instanceof Error ? error.message : 'Native resync failed'
        dispatch({ type: 'SET_CONNECTION', status: 'disconnected', errorMessage: message })
      } finally {
        resyncingRef.current = false
      }
    }

    const connect = async () => {
      dispatch({ type: 'SET_CONNECTION', status: 'loading' })

      try {
        const bootstrap = await clientRef.current.bootstrap()
        if (cancelled) {
          return
        }

        lastSequenceRef.current = bootstrap.lastSequence
        dispatch({
          type: 'APPLY_BOOTSTRAP',
          snapshot: bootstrap.snapshot,
          lastSequence: bootstrap.lastSequence,
          events: bootstrap.events,
        })

        const subscription = await clientRef.current.subscribe(
          (frame) => {
            const acceptance = evaluateStreamSequence(frame, lastSequenceRef.current)

            if (acceptance.kind === 'gap') {
              dispatch({
                type: 'SET_RESYNC_REASON',
                reason: `sequence gap: expected ${acceptance.expected}, received ${acceptance.received}`,
              })
              void resync()
              return
            }

            if (acceptance.kind === 'duplicate') {
              return
            }

            if (frame.payload.type === 'resyncRequired') {
              dispatch({ type: 'SET_RESYNC_REASON', reason: frame.payload.reason })
              void resync()
              return
            }

            lastSequenceRef.current = acceptance.nextSequence
            dispatch({ type: 'APPLY_FRAME', frame })
          },
          (error) => {
            if (isNativeClientError(error) && error.code === 'resync-required') {
              dispatch({ type: 'SET_RESYNC_REASON', reason: error.message })
              void resync()
              return
            }

            if (isNativeClientError(error) && error.code === 'protocol-incompatible') {
              dispatch({
                type: 'SET_PROTOCOL_INCOMPATIBLE',
                received: Number.NaN,
                expected: PROTOCOL_VERSION,
              })
              return
            }

            dispatch({
              type: 'SET_CONNECTION',
              status: 'disconnected',
              errorMessage: error.message,
            })
          },
        )

        subscriptionRef.current = subscription
      } catch (error) {
        if (cancelled) {
          return
        }

        if (isNativeClientError(error) && error.code === 'protocol-incompatible') {
          dispatch({
            type: 'SET_PROTOCOL_INCOMPATIBLE',
            received: Number.NaN,
            expected: PROTOCOL_VERSION,
          })
          return
        }

        const message = error instanceof Error ? error.message : 'Native connection failed'
        dispatch({ type: 'SET_CONNECTION', status: 'disconnected', errorMessage: message })
      }
    }

    void connect()

    return () => {
      cancelled = true
      const subscription = subscriptionRef.current
      subscriptionRef.current = null
      lastSequenceRef.current = null
      if (subscription) {
        void subscription.unsubscribe()
      }
    }
  }, [])

  const value = useMemo(
    () => ({
      state,
      dispatch,
      client: clientRef.current,
      prefersReducedMotion: state.reducedMotion,
    }),
    [state],
  )

  return <NativeStateContext.Provider value={value}>{children}</NativeStateContext.Provider>
}

export function useNativeState(): NativeStateContextValue {
  const context = useContext(NativeStateContext)

  if (!context) {
    throw new Error('useNativeState must be used within a NativeStateProvider')
  }

  return context
}
