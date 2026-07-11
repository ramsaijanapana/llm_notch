import { describe, expect, it } from 'vitest'
import type { AgentSession } from '../native/contracts.ts'
import type { NativeHistoryResponse } from '../native/types.ts'
import { persistedHistoryBundle } from './NativeSurfaces.tsx'

const sessions: AgentSession[] = [
  {
    id: 'cursor-a',
    source: 'cursor',
    externalSessionId: 'a',
    label: 'First task',
    status: 'running',
    attention: 'none',
    startedAtMs: 1,
    lastEventAtMs: 2,
  },
  {
    id: 'cursor-b',
    source: 'cursor',
    externalSessionId: 'b',
    label: 'Second task',
    status: 'running',
    attention: 'none',
    startedAtMs: 1,
    lastEventAtMs: 2,
  },
]

const emptySeries = {
  points: [],
  actualFirstMs: null,
  actualLastMs: null,
  totalPoints: 0,
  returnedPoints: 0,
  downsampled: false,
  truncated: false,
}

describe('persistedHistoryBundle', () => {
  it('keeps same-source sessions in distinct series', () => {
    const point = {
      atMs: 50,
      cpuHostPercent: 1,
      cpuCorePercent: 2,
      rssBytes: 3,
    }
    const response: NativeHistoryResponse = {
      range: '1h',
      sinceMs: 0,
      endMs: 100,
      host: emptySeries,
      aggregate: emptySeries,
      agents: sessions.map((session) => ({
        sessionId: session.id,
        points: [point],
        actualFirstMs: point.atMs,
        actualLastMs: point.atMs,
        totalPoints: 1,
        returnedPoints: 1,
        downsampled: false,
        truncated: false,
      })),
    }
    const bundle = persistedHistoryBundle(response, sessions)
    expect(bundle.perAgent).toHaveLength(2)
    expect(bundle.perAgent.map((series) => series.sessionId)).toEqual(['cursor-a', 'cursor-b'])
    expect(bundle.perAgent.map((series) => series.label)).toEqual([
      'Cursor — First task',
      'Cursor — Second task',
    ])
  })
})
