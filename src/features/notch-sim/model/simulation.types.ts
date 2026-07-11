export type SessionPhase = 'running' | 'needsApproval' | 'needsAnswer' | 'paused' | 'completed'

export type SessionEventKind = 'log' | 'question' | 'answer' | 'approval' | 'system'

export type SessionEventActor = 'user' | 'agent' | 'system'

export interface SessionEvent {
  id: string
  tick: number
  timestamp: number
  kind: SessionEventKind
  message: string
  actor: SessionEventActor
}

export interface AgentSession {
  id: string
  role: string
  task: string
  workspace: string
  phase: SessionPhase
  progress: number
  tokenCount: number
  costCents: number
  elapsedSeconds: number
  events: SessionEvent[]
  prompt?: string
  blockedReason?: string
}

export interface SimulationState {
  sessions: AgentSession[]
  selectedId: string
  expanded: boolean
  playing: boolean
  prefersReducedMotion: boolean
  terminalOpen: boolean
  tick: number
  announcement: string | null
  hasInteracted: boolean
}

export type SimulationAction =
  | { type: 'SELECT_SESSION'; sessionId: string }
  | { type: 'SET_EXPANDED'; expanded: boolean }
  | { type: 'TOGGLE_EXPANDED' }
  | { type: 'PLAY' }
  | { type: 'PAUSE' }
  | { type: 'TOGGLE_PLAYBACK' }
  | { type: 'APPROVE' }
  | { type: 'REJECT' }
  | { type: 'SUBMIT_ANSWER'; answer: string }
  | { type: 'RETRY' }
  | { type: 'ASK_AGENT'; question: string }
  | { type: 'JUMP' }
  | { type: 'CLOSE_TERMINAL' }
  | { type: 'TICK' }
  | { type: 'SET_REDUCED_MOTION'; value: boolean }
  | { type: 'RESET' }
