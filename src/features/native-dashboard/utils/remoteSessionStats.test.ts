import { describe, expect, it } from 'vitest'
import type { AgentSession } from '../../../native/contracts'
import { FIXED_NOW_MS, mockSessions } from '../fixtures/testFixtures'
import {
  remoteAttributedSessions,
  remoteHostIdFromWorkspaceLabel,
  summarizeRemoteIngestByHost,
} from './remoteSessionStats'

const now = FIXED_NOW_MS

function remoteSession(overrides: Partial<AgentSession> & Pick<AgentSession, 'id'>): AgentSession {
  return {
    source: 'cursor',
    externalSessionId: overrides.id,
    label: 'Remote task',
    workspaceLabel: 'remote:dev-box',
    status: 'running',
    attention: 'none',
    startedAtMs: now - 600_000,
    lastEventAtMs: now - 60_000,
    ...overrides,
  }
}

describe('remoteHostIdFromWorkspaceLabel', () => {
  it('extracts host id from remote workspace labels', () => {
    expect(remoteHostIdFromWorkspaceLabel('remote:dev-box')).toBe('dev-box')
  })

  it('returns null for local or missing labels', () => {
    expect(remoteHostIdFromWorkspaceLabel(undefined)).toBeNull()
    expect(remoteHostIdFromWorkspaceLabel('auth-service')).toBeNull()
    expect(remoteHostIdFromWorkspaceLabel('remote:')).toBeNull()
  })
})

describe('remoteAttributedSessions', () => {
  it('keeps only sessions with remote host attribution', () => {
    const sessions = [
      ...mockSessions,
      remoteSession({ id: 'remote-1' }),
      remoteSession({ id: 'remote-2', workspaceLabel: 'remote:lab-box' }),
    ]

    expect(remoteAttributedSessions(sessions)).toHaveLength(2)
  })
})

describe('summarizeRemoteIngestByHost', () => {
  it('aggregates counts and last event time per host', () => {
    const summaries = summarizeRemoteIngestByHost([
      remoteSession({ id: 'remote-active', status: 'running', lastEventAtMs: now - 30_000 }),
      remoteSession({
        id: 'remote-waiting',
        status: 'waiting',
        lastEventAtMs: now - 120_000,
      }),
      remoteSession({
        id: 'remote-done',
        status: 'completed',
        lastEventAtMs: now - 3_600_000,
        endedAtMs: now - 3_600_000,
      }),
      remoteSession({
        id: 'remote-lab',
        workspaceLabel: 'remote:lab-box',
        status: 'running',
        lastEventAtMs: now - 15_000,
      }),
    ])

    expect(summaries['dev-box']).toEqual({
      hostId: 'dev-box',
      totalSessions: 3,
      activeSessions: 2,
      lastEventAtMs: now - 30_000,
    })
    expect(summaries['lab-box']).toEqual({
      hostId: 'lab-box',
      totalSessions: 1,
      activeSessions: 1,
      lastEventAtMs: now - 15_000,
    })
  })

  it('returns an empty map when no remote sessions exist', () => {
    expect(summarizeRemoteIngestByHost(mockSessions)).toEqual({})
  })
})
