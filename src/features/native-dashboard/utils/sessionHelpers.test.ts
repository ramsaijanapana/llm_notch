import { describe, expect, it } from 'vitest'
import type { AgentSession, DecisionRequest } from '../../../native/contracts'
import { PREVIEW_ADAPTERS } from '../../../native/fixtures'
import {
  decisionMatchesSession,
  deriveSessionsEmptyMessage,
  findSessionForDecision,
} from './sessionHelpers'

const session: AgentSession = {
  id: 's-deadbeef',
  source: 'claudeCode',
  externalSessionId: 'claude-session-42',
  label: 'Write tests',
  status: 'waiting',
  attention: 'permission',
  startedAtMs: 1,
  lastEventAtMs: 2,
}

const decision: DecisionRequest = {
  id: 'dec-1',
  sessionId: 's-deadbeef',
  source: 'claudeCode',
  kind: 'permission',
  summary: 'Allow npm test',
  hasActionablePayload: true,
  createdAtMs: 3,
}

describe('decisionMatchesSession', () => {
  it('matches internal session ids', () => {
    expect(decisionMatchesSession(decision, session)).toBe(true)
  })

  it('matches external session ids for legacy broker payloads', () => {
    expect(decisionMatchesSession({ ...decision, sessionId: 'claude-session-42' }, session)).toBe(
      true,
    )
  })

  it('rejects unrelated sessions', () => {
    expect(decisionMatchesSession({ ...decision, sessionId: 'other-session' }, session)).toBe(false)
  })
})

describe('findSessionForDecision', () => {
  it('returns the matching session when present', () => {
    expect(findSessionForDecision([session], decision)).toEqual(session)
  })
})

describe('deriveSessionsEmptyMessage', () => {
  it('guides users when hooks are installed but waiting for traffic', () => {
    const cursorAdapter = PREVIEW_ADAPTERS.find((adapter) => adapter.source === 'cursor')
    if (!cursorAdapter) throw new Error('cursor adapter missing')

    const message = deriveSessionsEmptyMessage(PREVIEW_ADAPTERS, {
      checkedAtMs: Date.now(),
      adapters: [
        {
          source: 'cursor',
          status: 'waitingFirstEvent',
          probes: [],
          capabilities: cursorAdapter,
        },
      ],
    })

    expect(message).toMatch(/Hooks are installed/i)
    expect(message).toMatch(/verify live traffic/i)
  })

  it('guides users when hooks still need installation', () => {
    const message = deriveSessionsEmptyMessage(PREVIEW_ADAPTERS, {
      checkedAtMs: Date.now(),
      adapters: PREVIEW_ADAPTERS.map((adapter) => ({
        source: adapter.source,
        status: 'notInstalled' as const,
        probes: [],
        capabilities: adapter,
      })),
    })

    expect(message).toMatch(/Install or repair llm_notch hooks/i)
  })
})
