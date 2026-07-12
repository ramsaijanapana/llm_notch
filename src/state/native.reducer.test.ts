import { describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from '../native/contracts.ts'
import { createPreviewSnapshot, PREVIEW_EVENTS } from '../native/fixtures.ts'
import { MAX_EVENTS, MAX_EVENTS_PER_SESSION } from './constants.ts'
import { createInitialNativeState, nativeReducer } from './native.reducer.ts'

describe('nativeReducer', () => {
  it('applies bootstrap snapshots and selects the first session by default', () => {
    const snapshot = createPreviewSnapshot()
    const next = nativeReducer(createInitialNativeState('preview'), {
      type: 'APPLY_BOOTSTRAP',
      snapshot,
      lastSequence: 4,
      events: PREVIEW_EVENTS,
    })

    expect(next.connection).toBe('connected')
    expect(next.protocolVersion).toBe(PROTOCOL_VERSION)
    expect(next.lastSequence).toBe(4)
    expect(next.sessionOrder).toHaveLength(snapshot.sessions.length)
    expect(next.selectedSessionId).toBe(snapshot.sessions[0]?.id)
    expect(next.reducedMotion).toBe(snapshot.settings.reducedMotion)
    expect(next.events).toHaveLength(PREVIEW_EVENTS.length)
  })

  it('marks incompatible protocol versions', () => {
    const snapshot = createPreviewSnapshot()
    snapshot.protocolVersion = PROTOCOL_VERSION + 1

    const next = nativeReducer(createInitialNativeState('preview'), {
      type: 'APPLY_BOOTSTRAP',
      snapshot,
      lastSequence: 0,
      events: [],
    })

    expect(next.connection).toBe('incompatible-protocol')
    expect(next.errorMessage).toContain(String(PROTOCOL_VERSION + 1))
  })

  it('bounds global and per-session events', () => {
    let state = createInitialNativeState('preview')

    for (let index = 0; index < MAX_EVENTS + 12; index += 1) {
      state = nativeReducer(state, {
        type: 'APPLY_FRAME',
        frame: {
          sequence: index + 1,
          emittedAtMs: Date.now(),
          payload: {
            type: 'sessionEvent',
            event: {
              id: `evt-${index}`,
              sessionId: 'sess-cursor-refactor',
              sequence: index + 1,
              occurredAtMs: Date.now(),
              kind: 'status',
              level: 'info',
              summary: `event ${index}`,
            },
          },
        },
      })
    }

    expect(state.events).toHaveLength(MAX_EVENTS)
    expect(state.eventsBySession['sess-cursor-refactor']).toHaveLength(MAX_EVENTS_PER_SESSION)
    expect(state.events[0]?.summary).toBe('event 12')
  })

  it('latest metrics frame wins in state', () => {
    const first = createPreviewSnapshot()
    let state = nativeReducer(createInitialNativeState('preview'), {
      type: 'APPLY_BOOTSTRAP',
      snapshot: first,
      lastSequence: 1,
      events: [],
    })

    state = nativeReducer(state, {
      type: 'APPLY_FRAME',
      frame: {
        sequence: 2,
        emittedAtMs: Date.now(),
        payload: {
          type: 'metrics',
          metrics: {
            host: {
              atMs: Date.now(),
              cpuHostPercent: 10,
              usedMemoryBytes: 1,
              totalMemoryBytes: 2,
              visibleProcessCount: 3,
              diskReadBytesPerSec: 4,
              diskWriteBytesPerSec: 5,
            },
            aggregate: {
              atMs: Date.now(),
              cpuCorePercent: 11,
              cpuHostPercent: 12,
              rssBytes: 13,
              runtimeMs: 14,
              processCount: 15,
              readBytesPerSec: 16,
              writeBytesPerSec: 17,
              quality: {
                attribution: 'exact',
                cpu: 'available',
                io: 'disk',
              },
              activeSessions: 1,
              attentionSessions: 0,
            },
            agents: {},
          },
        },
      },
    })

    state = nativeReducer(state, {
      type: 'APPLY_FRAME',
      frame: {
        sequence: 3,
        emittedAtMs: Date.now(),
        payload: {
          type: 'metrics',
          metrics: {
            host: {
              atMs: Date.now(),
              cpuHostPercent: 99,
              usedMemoryBytes: 1,
              totalMemoryBytes: 2,
              visibleProcessCount: 3,
              diskReadBytesPerSec: 4,
              diskWriteBytesPerSec: 5,
            },
            aggregate: {
              atMs: Date.now(),
              cpuCorePercent: 88,
              cpuHostPercent: 77,
              rssBytes: 13,
              runtimeMs: 14,
              processCount: 15,
              readBytesPerSec: 16,
              writeBytesPerSec: 17,
              quality: {
                attribution: 'exact',
                cpu: 'available',
                io: 'disk',
              },
              activeSessions: 1,
              attentionSessions: 0,
            },
            agents: {},
          },
        },
      },
    })

    expect(state.metrics?.host.cpuHostPercent).toBe(99)
    expect(state.metrics?.aggregate.cpuCorePercent).toBe(88)
    expect(state.sessions['sess-cursor-refactor']?.latestMetric).toBeUndefined()
  })

  it('keeps snapshot sessions in sync with session upserts and removes', () => {
    const snapshot = createPreviewSnapshot()
    let state = nativeReducer(createInitialNativeState('preview'), {
      type: 'APPLY_BOOTSTRAP',
      snapshot,
      lastSequence: 1,
      events: [],
    })
    const liveSession = snapshot.sessions[0]
    if (!liveSession) throw new Error('preview session missing')

    state = nativeReducer(state, {
      type: 'APPLY_FRAME',
      frame: {
        sequence: 2,
        emittedAtMs: Date.now(),
        payload: {
          type: 'sessionUpsert',
          session: {
            ...liveSession,
            id: 'sess-live-upsert',
            label: 'Live upsert',
          },
        },
      },
    })

    expect(state.sessionOrder).toContain('sess-live-upsert')
    expect(state.snapshot?.sessions.some((session) => session.id === 'sess-live-upsert')).toBe(true)

    state = nativeReducer(state, {
      type: 'APPLY_FRAME',
      frame: {
        sequence: 3,
        emittedAtMs: Date.now(),
        payload: { type: 'sessionRemove', sessionId: 'sess-live-upsert' },
      },
    })

    expect(state.sessionOrder).not.toContain('sess-live-upsert')
    expect(state.snapshot?.sessions.some((session) => session.id === 'sess-live-upsert')).toBe(
      false,
    )
  })

  it('merges and clears authoritative per-session metrics', () => {
    const snapshot = createPreviewSnapshot()
    let state = nativeReducer(createInitialNativeState('preview'), {
      type: 'APPLY_BOOTSTRAP',
      snapshot,
      lastSequence: 0,
      events: [],
    })
    const sample = snapshot.sessions[0]?.latestMetric
    const host = snapshot.host
    const aggregate = snapshot.aggregate
    if (!sample || !host || !aggregate) throw new Error('preview metrics missing')
    const baseMetrics = {
      host,
      aggregate,
    }

    state = nativeReducer(state, {
      type: 'APPLY_FRAME',
      frame: {
        sequence: 1,
        emittedAtMs: sample.atMs,
        payload: {
          type: 'metrics',
          metrics: {
            ...baseMetrics,
            agents: {
              'sess-cursor-refactor': { ...sample, cpuCorePercent: 73 },
            },
          },
        },
      },
    })
    expect(state.sessions['sess-cursor-refactor']?.latestMetric?.cpuCorePercent).toBe(73)
    expect(
      state.snapshot?.sessions.find((session) => session.id === 'sess-cursor-refactor')
        ?.latestMetric?.cpuCorePercent,
    ).toBe(73)

    state = nativeReducer(state, {
      type: 'APPLY_FRAME',
      frame: {
        sequence: 2,
        emittedAtMs: sample.atMs + 1,
        payload: { type: 'metrics', metrics: { ...baseMetrics, agents: {} } },
      },
    })
    expect(state.sessions['sess-cursor-refactor']?.latestMetric).toBeUndefined()
  })
})
