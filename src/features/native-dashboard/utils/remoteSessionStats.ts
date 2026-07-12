import type { AgentSession } from '../../../native/contracts'
import { activeSessions } from './sessionHelpers'

export const REMOTE_WORKSPACE_PREFIX = 'remote:'

export interface RemoteHostIngestSummary {
  hostId: string
  totalSessions: number
  activeSessions: number
  lastEventAtMs: number | null
}

export function remoteHostIdFromWorkspaceLabel(
  workspaceLabel: string | undefined,
): string | null {
  if (!workspaceLabel?.startsWith(REMOTE_WORKSPACE_PREFIX)) {
    return null
  }
  const hostId = workspaceLabel.slice(REMOTE_WORKSPACE_PREFIX.length).trim()
  return hostId.length > 0 ? hostId : null
}

export function remoteAttributedSessions(sessions: AgentSession[]): AgentSession[] {
  return sessions.filter(
    (session) => remoteHostIdFromWorkspaceLabel(session.workspaceLabel) != null,
  )
}

export function summarizeRemoteIngestByHost(
  sessions: AgentSession[],
): Record<string, RemoteHostIngestSummary> {
  const byHost = new Map<string, AgentSession[]>()

  for (const session of remoteAttributedSessions(sessions)) {
    const hostId = remoteHostIdFromWorkspaceLabel(session.workspaceLabel)
    if (!hostId) {
      continue
    }
    const existing = byHost.get(hostId) ?? []
    existing.push(session)
    byHost.set(hostId, existing)
  }

  const summaries: Record<string, RemoteHostIngestSummary> = {}
  for (const [hostId, hostSessions] of byHost) {
    const active = activeSessions(hostSessions)
    const lastEventAtMs = hostSessions.reduce<number | null>((latest, session) => {
      if (latest == null || session.lastEventAtMs > latest) {
        return session.lastEventAtMs
      }
      return latest
    }, null)

    summaries[hostId] = {
      hostId,
      totalSessions: hostSessions.length,
      activeSessions: active.length,
      lastEventAtMs,
    }
  }

  return summaries
}
