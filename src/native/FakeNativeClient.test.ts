import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFakeNativeClient } from './FakeNativeClient.ts'
import type { ConnectorHealthEntry } from './types.ts'

describe('FakeNativeClient', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('bootstraps a realistic preview snapshot with three sessions', async () => {
    const client = createFakeNativeClient()
    const bootstrap = await client.bootstrap()

    expect(client.mode).toBe('preview')
    expect(bootstrap.snapshot.sessions).toHaveLength(3)
    expect(bootstrap.snapshot.host).toBeDefined()
    expect(bootstrap.snapshot.aggregate).toBeDefined()
  })

  it('streams live metric updates and cleans up on unsubscribe', async () => {
    vi.useFakeTimers()
    const client = createFakeNativeClient()
    const frames: number[] = []

    const subscription = await client.subscribe(
      (frame) => {
        if (frame.payload.type === 'metrics') {
          frames.push(frame.sequence)
        }
      },
      () => {},
    )

    vi.advanceTimersByTime(2_500)
    await subscription.unsubscribe()
    vi.advanceTimersByTime(2_500)

    expect(frames.length).toBeGreaterThan(0)
    const afterUnsubscribe = frames.length
    vi.advanceTimersByTime(2_500)
    expect(frames.length).toBe(afterUnsubscribe)
  })

  it('detects connectors and previews apply flow', async () => {
    const client = createFakeNativeClient()
    const detected = await client.detectConnectors()
    expect(detected.length).toBeGreaterThan(0)
    expect(detected.some((entry) => entry.source === 'qwen')).toBe(true)
    expect(detected.some((entry) => entry.source === 'copilotCli')).toBe(true)

    const preview = await client.previewConnector('cursor')
    expect(preview.files.length).toBeGreaterThan(0)

    const qwenPreview = await client.previewConnector('qwen')
    expect(qwenPreview.files[0]?.displayPath).toBe('~/.qwen/settings.json')

    const result = await client.applyConnectorChange(preview.planId)
    expect(result.fileResults.some((file) => file.outcome === 'applied')).toBe(true)

    const backups = await client.listConnectorBackups()
    expect(backups.length).toBeGreaterThan(0)
  })

  it('reports integration health for preview adapters', async () => {
    const client = createFakeNativeClient()
    const health = await client.getIntegrationHealth()

    expect(health.adapters).toHaveLength(7)
    expect(health.adapters.some((entry) => entry.source === 'gemini')).toBe(true)
    expect(health.adapters.every((entry: ConnectorHealthEntry) => entry.probes.length >= 4)).toBe(
      true,
    )
    expect(['notInstalled', 'waitingFirstEvent', 'actionNeeded', 'driftDetected']).toContain(
      health.adapters[0]?.status,
    )
  })

  it('lists the honest 25-agent catalog', async () => {
    const catalog = await createFakeNativeClient().listAgentCatalog()
    expect(catalog).toHaveLength(25)
    expect(
      catalog.filter((entry) => entry.maturity === 'verifiedCurrent').map((entry) => entry.id),
    ).toEqual([
      'claude-code',
      'codex',
      'gemini-cli',
      'antigravity-cli',
      'cursor',
      'qwen',
      'copilot',
    ])
    expect(
      catalog
        .filter((entry) => entry.maturity === 'declaredUnverified')
        .every((entry) => entry.capabilities.length === 0),
    ).toBe(true)
  })

  it('acknowledges local attention without simulating vendor approvals', async () => {
    const client = createFakeNativeClient()
    await client.acknowledgeLocalAttention('sess-claude-review')

    expect(client.wasAttentionAcknowledged('sess-claude-review')).toBe(true)
  })

  it('reports honest unavailable remote backend state', async () => {
    const client = createFakeNativeClient()
    await expect(client.listRemoteHosts()).resolves.toEqual([])
    await expect(client.getRemoteBackendStatus()).resolves.toMatchObject({
      availability: 'unavailable',
    })
    await expect(client.previewRemoteDeploy('dev-box')).rejects.toMatchObject({
      code: 'remote-backend-unavailable',
    })
    await expect(client.getRemoteConnectionStatus('dev-box')).resolves.toMatchObject({
      connectionState: 'disconnected',
      availability: 'unavailable',
    })
  })

  it('stores remote hosts in memory without fabricating connection state', async () => {
    const client = createFakeNativeClient()
    const saved = await client.upsertRemoteHost({
      id: 'dev-box',
      destination: 'dev@example.internal',
      port: 2222,
      identityFile: null,
      hostKeyPolicy: 'strict',
      connectTimeoutSeconds: 10,
    })
    expect(saved).toMatchObject({
      config: {
        id: 'dev-box',
        destination: 'dev@example.internal',
        port: 2222,
      },
      availability: 'unavailable',
      connectionState: 'disconnected',
    })
    await expect(client.listRemoteHosts()).resolves.toHaveLength(1)
    await client.removeRemoteHost('dev-box')
    await expect(client.listRemoteHosts()).resolves.toEqual([])
  })

  it('delivers remote connection change events and cleans up on unsubscribe', async () => {
    const client = createFakeNativeClient()
    await client.upsertRemoteHost({
      id: 'dev-box',
      destination: 'dev@example.internal',
      hostKeyPolicy: 'strict',
      connectTimeoutSeconds: 10,
    })

    const updates: string[] = []
    const subscription = await client.subscribeRemoteConnectionChanges((status) => {
      updates.push(status.connectionState as string)
    })

    client.simulateRemoteConnectionChange({
      hostId: 'dev-box',
      availability: 'available',
      connectionState: 'connecting',
      message: 'Opening SSH session',
    })
    client.simulateRemoteConnectionChange({
      hostId: 'dev-box',
      availability: 'available',
      connectionState: 'streaming',
      message: null,
    })

    await subscription.unsubscribe()

    client.simulateRemoteConnectionChange({
      hostId: 'dev-box',
      availability: 'available',
      connectionState: 'disconnected',
      message: null,
    })

    expect(updates).toEqual(['connecting', 'streaming'])
  })
})
