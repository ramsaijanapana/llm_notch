import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from './contracts.ts'
import { createPreviewSnapshot } from './fixtures.ts'
import { createTauriNativeClient } from './TauriNativeClient.ts'

describe('TauriNativeClient', () => {
  afterEach(() => {
    clearMocks()
  })

  it('validates protocol version during bootstrap', async () => {
    mockIPC((cmd) => {
      if (cmd === 'bootstrap') {
        const snapshot = createPreviewSnapshot()
        snapshot.protocolVersion = PROTOCOL_VERSION + 9
        return { snapshot, lastSequence: 0 }
      }
      return null
    })

    const client = createTauriNativeClient()
    await expect(client.bootstrap()).rejects.toMatchObject({
      code: 'protocol-incompatible',
    })
  })

  it('delivers every sequence for provider-side validation', async () => {
    let sequence = 0

    mockIPC((cmd, payload) => {
      if (cmd === 'bootstrap') {
        return { snapshot: createPreviewSnapshot(), lastSequence: 0 }
      }

      if (cmd === 'subscribe_stream') {
        const onEvent = (payload as { onEvent?: { onmessage?: (frame: unknown) => void } }).onEvent
        const snapshot = createPreviewSnapshot()
        const host = snapshot.host
        const aggregate = snapshot.aggregate
        if (!host || !aggregate) {
          throw new Error('Preview snapshot missing host metrics')
        }

        queueMicrotask(() => {
          onEvent?.onmessage?.({
            sequence: ++sequence,
            emittedAtMs: 1,
            payload: {
              type: 'metrics',
              metrics: {
                host,
                aggregate,
                agents: {},
              },
            },
          })
          onEvent?.onmessage?.({
            sequence: ++sequence,
            emittedAtMs: 2,
            payload: {
              type: 'metrics',
              metrics: {
                host: {
                  atMs: 2,
                  cpuHostPercent: 42,
                  usedMemoryBytes: 0,
                  totalMemoryBytes: 0,
                  visibleProcessCount: 0,
                  diskReadBytesPerSec: 0,
                  diskWriteBytesPerSec: 0,
                },
                aggregate,
                agents: {},
              },
            },
          })
        })
        return 'sub-1'
      }

      if (cmd === 'unsubscribe_stream') {
        return null
      }

      return null
    })

    const client = createTauriNativeClient()
    await client.bootstrap()

    const metricsFrames: number[] = []
    const subscription = await client.subscribe(
      (frame) => {
        if (frame.payload.type === 'metrics') {
          metricsFrames.push(frame.sequence)
        }
      },
      () => {},
    )

    await new Promise((resolve) => {
      setTimeout(resolve, 10)
    })

    expect(metricsFrames).toEqual([1, 2])
    await subscription.unsubscribe()
  })

  it('unsubscribes cleanly and allows a new subscription', async () => {
    let unsubscribeCalls = 0

    mockIPC((cmd) => {
      if (cmd === 'bootstrap') {
        return { snapshot: createPreviewSnapshot(), lastSequence: 0 }
      }
      if (cmd === 'subscribe_stream') {
        return 'sub-42'
      }
      if (cmd === 'unsubscribe_stream') {
        unsubscribeCalls += 1
        return null
      }
      return null
    })

    const client = createTauriNativeClient()
    await client.bootstrap()
    const subscription = await client.subscribe(
      () => {},
      () => {},
    )
    await subscription.unsubscribe()

    expect(unsubscribeCalls).toBe(1)

    const secondSubscription = await client.subscribe(
      () => {},
      () => {},
    )
    expect(secondSubscription).toBeDefined()
    await secondSubscription.unsubscribe()
    expect(unsubscribeCalls).toBe(2)
  })

  it('loads persisted history and scoped display options', async () => {
    mockIPC((cmd, payload) => {
      if (cmd === 'get_history') {
        expect(payload).toMatchObject({ range: '24h' })
        return {
          range: '24h',
          sinceMs: 1,
          endMs: 2,
          host: {
            points: [],
            actualFirstMs: null,
            actualLastMs: null,
            totalPoints: 0,
            returnedPoints: 0,
            downsampled: false,
            truncated: false,
          },
          aggregate: {
            points: [],
            actualFirstMs: null,
            actualLastMs: null,
            totalPoints: 0,
            returnedPoints: 0,
            downsampled: false,
            truncated: false,
          },
          agents: [],
        }
      }
      if (cmd === 'list_displays') {
        return [{ id: 'display-1', label: 'Built-in', primary: true }]
      }
      if (cmd === 'get_session_events') {
        expect(payload).toMatchObject({
          sessionId: 'session-1',
          beforeSequence: 101,
          limit: 50,
        })
        return {
          sessionId: 'session-1',
          events: [],
          nextBeforeSequence: 51,
          hasMore: true,
        }
      }
      return null
    })
    const client = createTauriNativeClient()
    await expect(client.getHistory('24h')).resolves.toMatchObject({ range: '24h' })
    await expect(client.listDisplays()).resolves.toEqual([
      { id: 'display-1', label: 'Built-in', primary: true },
    ])
    await expect(client.getSessionEvents('session-1', 101, 50)).resolves.toMatchObject({
      nextBeforeSequence: 51,
      hasMore: true,
    })
  })
})
