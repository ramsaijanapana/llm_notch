import { describe, expect, it } from 'vitest'
import type { StreamFrame } from './contracts.ts'
import { coalesceStreamFrames, evaluateStreamSequence } from './streamProcessor.ts'

function frame(sequence: number, type: 'metrics' | 'heartbeat' = 'metrics'): StreamFrame {
  if (type === 'heartbeat') {
    return { sequence, emittedAtMs: sequence, payload: { type: 'heartbeat' } }
  }

  return {
    sequence,
    emittedAtMs: sequence,
    payload: {
      type: 'metrics',
      metrics: {
        host: {
          atMs: sequence,
          cpuHostPercent: sequence,
          usedMemoryBytes: 0,
          totalMemoryBytes: 0,
          visibleProcessCount: 0,
          diskReadBytesPerSec: 0,
          diskWriteBytesPerSec: 0,
        },
        aggregate: {
          atMs: sequence,
          cpuCorePercent: sequence,
          cpuHostPercent: sequence,
          rssBytes: 0,
          runtimeMs: 0,
          processCount: 0,
          readBytesPerSec: 0,
          writeBytesPerSec: 0,
          quality: {
            attribution: 'exact',
            cpu: 'available',
            io: 'disk',
          },
          activeSessions: 0,
          attentionSessions: 0,
        },
        agents: {},
      },
    },
  }
}

describe('evaluateStreamSequence', () => {
  it('accepts the first frame and strict increments', () => {
    expect(evaluateStreamSequence(frame(1), null)).toEqual({
      kind: 'accept',
      nextSequence: 1,
    })
    expect(evaluateStreamSequence(frame(2), 1)).toEqual({
      kind: 'accept',
      nextSequence: 2,
    })
  })

  it('treats replays as duplicates', () => {
    expect(evaluateStreamSequence(frame(2), 2)).toEqual({ kind: 'duplicate' })
    expect(evaluateStreamSequence(frame(1), 4)).toEqual({ kind: 'duplicate' })
  })

  it('detects sequence gaps for resync', () => {
    expect(evaluateStreamSequence(frame(5), 2)).toEqual({
      kind: 'gap',
      expected: 3,
      received: 5,
    })
  })
})

describe('coalesceStreamFrames', () => {
  it('keeps only the latest metrics frame between other payload types', () => {
    const coalesced = coalesceStreamFrames([
      frame(1, 'heartbeat'),
      frame(2),
      frame(3),
      frame(4),
      frame(5, 'heartbeat'),
    ])

    expect(coalesced).toHaveLength(3)
    expect(coalesced[0]?.payload.type).toBe('heartbeat')
    expect(coalesced[1]?.sequence).toBe(4)
    expect(coalesced[2]?.payload.type).toBe('heartbeat')
  })
})
