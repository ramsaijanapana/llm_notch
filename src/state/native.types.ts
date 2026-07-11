import type {
  AdapterCapabilities,
  AgentSession,
  AppSnapshot,
  MetricsFrame,
  PublicSettings,
  SessionEvent,
  StreamFrame,
} from '../native/contracts.ts'
import type {
  NativeClientMode,
  NativeDisplayOption,
  NativeHistoryRange,
  NativeHistoryResponse,
} from '../native/types.ts'

export type ConnectionStatus =
  | 'loading'
  | 'connected'
  | 'disconnected'
  | 'resyncing'
  | 'incompatible-protocol'

export interface NativeState {
  clientMode: NativeClientMode
  connection: ConnectionStatus
  protocolVersion: number | null
  lastSequence: number | null
  resyncReason: string | null
  errorMessage: string | null

  snapshot: AppSnapshot | null
  sessions: Record<string, AgentSession>
  sessionOrder: string[]
  events: SessionEvent[]
  eventsBySession: Record<string, SessionEvent[]>
  metrics: MetricsFrame | null
  adapters: AdapterCapabilities[]
  settings: PublicSettings | null
  selectedSessionId: string | null
  reducedMotion: boolean
  historyRange: NativeHistoryRange
  historyStatus: 'idle' | 'loading' | 'ready' | 'empty' | 'error'
  history: NativeHistoryResponse | null
  historyError: string | null
  displays: NativeDisplayOption[]
  displayStatus: 'idle' | 'loading' | 'ready' | 'error'
  displayError: string | null
}

export type NativeAction =
  | { type: 'SET_CLIENT_MODE'; mode: NativeClientMode }
  | { type: 'SET_CONNECTION'; status: ConnectionStatus; errorMessage?: string }
  | { type: 'SET_PROTOCOL_INCOMPATIBLE'; received: number; expected: number }
  | {
      type: 'APPLY_BOOTSTRAP'
      snapshot: AppSnapshot
      lastSequence: number
      events: SessionEvent[]
    }
  | { type: 'APPLY_FRAME'; frame: StreamFrame }
  | { type: 'SET_RESYNC_REASON'; reason: string }
  | { type: 'CLEAR_RESYNC' }
  | { type: 'SET_SELECTED_SESSION'; sessionId: string | null }
  | { type: 'SET_HISTORY_RANGE'; range: NativeHistoryRange }
  | { type: 'SET_HISTORY_LOADING'; range: NativeHistoryRange }
  | { type: 'SET_HISTORY'; history: NativeHistoryResponse }
  | { type: 'SET_HISTORY_ERROR'; range: NativeHistoryRange; message: string }
  | { type: 'SET_DISPLAYS_LOADING' }
  | { type: 'SET_DISPLAYS'; displays: NativeDisplayOption[] }
  | { type: 'SET_DISPLAYS_ERROR'; message: string }
  | { type: 'RESET' }

export interface NativeStore {
  state: NativeState
  dispatch: (action: NativeAction) => void
}
