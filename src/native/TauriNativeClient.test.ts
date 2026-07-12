import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { NATIVE_EVENTS } from './commands.ts'
import { PROTOCOL_VERSION } from './contracts.ts'
import { createPreviewSnapshot } from './fixtures.ts'
import { createTauriNativeClient } from './TauriNativeClient.ts'

const listenMock = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

describe('TauriNativeClient', () => {
  afterEach(() => {
    clearMocks()
    listenMock.mockReset()
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

  it('invokes the read-only agent catalog command', async () => {
    mockIPC((cmd) => {
      if (cmd === 'list_agent_catalog') {
        return [
          {
            id: 'opencode',
            displayName: 'OpenCode',
            aliases: [],
            executableNames: [],
            adapterFamily: 'undetermined',
            maturity: 'declaredUnverified',
            capabilities: [],
            configTargets: [],
          },
        ]
      }
      return null
    })

    const catalog = await createTauriNativeClient().listAgentCatalog()
    expect(catalog).toHaveLength(1)
    expect(catalog[0]).toMatchObject({ id: 'opencode', maturity: 'declaredUnverified' })
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

  it('invokes remote lifecycle commands with honest unavailable responses', async () => {
    mockIPC((cmd, payload) => {
      if (cmd === 'list_remote_hosts') {
        return []
      }
      if (cmd === 'get_remote_backend_status') {
        return {
          availability: 'unavailable',
          message: 'SSH relay backend is not available in this build.',
        }
      }
      if (cmd === 'preview_remote_deploy') {
        expect(payload).toMatchObject({ hostId: 'dev-box' })
        throw 'SSH relay backend is not available in this build.'
      }
      if (cmd === 'start_remote_relay') {
        expect(payload).toMatchObject({ hostId: 'dev-box' })
        throw 'SSH relay backend is not available in this build.'
      }
      if (cmd === 'get_remote_connection_status') {
        expect(payload).toMatchObject({ hostId: 'dev-box' })
        return {
          hostId: 'dev-box',
          availability: 'unavailable',
          connectionState: 'disconnected',
          message: 'SSH relay backend is not available in this build.',
        }
      }
      if (cmd === 'upsert_remote_host') {
        expect(payload).toMatchObject({
          config: {
            id: 'dev-box',
            destination: 'dev@example.internal',
          },
        })
        return {
          config: {
            id: 'dev-box',
            destination: 'dev@example.internal',
            hostKeyPolicy: 'strict',
            connectTimeoutSeconds: 10,
          },
          availability: 'unavailable',
          connectionState: 'disconnected',
        }
      }
      if (cmd === 'remove_remote_host') {
        expect(payload).toMatchObject({ hostId: 'dev-box' })
        return null
      }
      return null
    })

    const client = createTauriNativeClient()
    await expect(client.listRemoteHosts()).resolves.toEqual([])
    await expect(client.getRemoteBackendStatus()).resolves.toMatchObject({
      availability: 'unavailable',
    })
    await expect(client.previewRemoteDeploy('dev-box')).rejects.toThrow()
    await expect(client.startRemoteRelay('dev-box')).rejects.toThrow()
    await expect(client.getRemoteConnectionStatus('dev-box')).resolves.toMatchObject({
      connectionState: 'disconnected',
    })
    await expect(
      client.upsertRemoteHost({
        id: 'dev-box',
        destination: 'dev@example.internal',
        hostKeyPolicy: 'strict',
        connectTimeoutSeconds: 10,
      }),
    ).resolves.toMatchObject({
      config: { id: 'dev-box' },
      connectionState: 'disconnected',
    })
    await expect(client.removeRemoteHost('dev-box')).resolves.toBeUndefined()
  })

  it('subscribes to remote connection change events and unsubscribes cleanly', async () => {
    const unlisten = vi.fn()
    let eventHandler: ((event: { payload: unknown }) => void) | undefined
    listenMock.mockImplementation(async (eventName, handler) => {
      expect(eventName).toBe(NATIVE_EVENTS.remoteConnectionChanged)
      eventHandler = handler
      return unlisten
    })

    const client = createTauriNativeClient()
    const updates: string[] = []
    const subscription = await client.subscribeRemoteConnectionChanges((status) => {
      updates.push(status.connectionState as string)
    })

    eventHandler?.({
      payload: {
        hostId: 'dev-box',
        availability: 'available',
        connectionState: 'connecting',
        message: 'Opening SSH session',
      },
    })
    eventHandler?.({
      payload: {
        hostId: 'dev-box',
        availability: 'available',
        connectionState: 'streaming',
        message: null,
      },
    })

    await subscription.unsubscribe()
    expect(unlisten).toHaveBeenCalledTimes(1)
    expect(updates).toEqual(['connecting', 'streaming'])
  })
})
