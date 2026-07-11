import {
  AlertCircle,
  CheckCircle2,
  CircleHelp,
  Loader2,
  type LucideIcon,
  PauseCircle,
} from 'lucide-react'
import type { AgentSession, SessionPhase } from '../model/simulation.types'

export type PhaseMeta = {
  label: string
  Icon: LucideIcon
  tone: 'success' | 'warning' | 'error' | 'info' | 'neutral'
}

const PHASE_META: Record<SessionPhase, PhaseMeta> = {
  running: { label: 'Running', Icon: Loader2, tone: 'info' },
  needsApproval: { label: 'Needs approval', Icon: AlertCircle, tone: 'warning' },
  needsAnswer: { label: 'Needs answer', Icon: CircleHelp, tone: 'warning' },
  paused: { label: 'Paused', Icon: PauseCircle, tone: 'error' },
  completed: { label: 'Completed', Icon: CheckCircle2, tone: 'success' },
}

export function getPhaseMeta(phase: SessionPhase): PhaseMeta {
  return PHASE_META[phase]
}

export function countActiveSessions(sessions: AgentSession[]): number {
  return sessions.filter((session) => session.phase === 'running').length
}

export function countAttentionSessions(sessions: AgentSession[]): number {
  return sessions.filter(
    (session) => session.phase === 'needsApproval' || session.phase === 'needsAnswer',
  ).length
}

export function findSelectedSession(
  sessions: AgentSession[],
  selectedId: string,
): AgentSession | undefined {
  return sessions.find((session) => session.id === selectedId)
}

export function formatCost(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`
}

export function formatElapsed(seconds: number): string {
  const minutes = Math.floor(seconds / 60)
  const remainder = seconds % 60
  return `${minutes}:${remainder.toString().padStart(2, '0')}`
}
