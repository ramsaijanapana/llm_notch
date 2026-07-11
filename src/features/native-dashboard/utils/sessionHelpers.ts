import type { AdapterCapabilities, AgentSession } from '../../../native/contracts'

export function sessionsNeedingAttention(sessions: AgentSession[]): AgentSession[] {
  return sessions
    .filter((session) => session.attention !== 'none')
    .sort((left, right) => right.lastEventAtMs - left.lastEventAtMs)
}

export function activeSessions(sessions: AgentSession[]): AgentSession[] {
  return sessions
    .filter((session) => session.status === 'running' || session.status === 'waiting')
    .sort((left, right) => right.lastEventAtMs - left.lastEventAtMs)
}

export function recentSessions(sessions: AgentSession[]): AgentSession[] {
  return [...sessions].sort((left, right) => right.lastEventAtMs - left.lastEventAtMs)
}

export function findAdapterForSession(
  adapters: AdapterCapabilities[],
  session: AgentSession,
): AdapterCapabilities | undefined {
  return adapters.find((adapter) => adapter.source === session.source)
}

export function isNotifyOnlyAdapter(adapter: AdapterCapabilities | undefined): boolean {
  if (!adapter) {
    return true
  }
  return !adapter.decisionResponse
}
