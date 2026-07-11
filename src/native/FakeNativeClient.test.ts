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

    const preview = await client.previewConnector('cursor')
    expect(preview.files.length).toBeGreaterThan(0)

    const result = await client.applyConnectorChange(preview.planId)
    expect(result.fileResults.some((file) => file.outcome === 'applied')).toBe(true)

    const backups = await client.listConnectorBackups()
    expect(backups.length).toBeGreaterThan(0)
  })

  it('reports integration health for preview adapters', async () => {
    const client = createFakeNativeClient()
    const health = await client.getIntegrationHealth()

    expect(health.adapters).toHaveLength(3)
    expect(health.adapters.every((entry: ConnectorHealthEntry) => entry.probes.length >= 4)).toBe(
      true,
    )
    expect(['notInstalled', 'waitingFirstEvent', 'actionNeeded']).toContain(
      health.adapters[0]?.status,
    )
  })

  it('acknowledges local attention without simulating vendor approvals', async () => {
    const client = createFakeNativeClient()
    await client.acknowledgeLocalAttention('sess-claude-review')

    expect(client.wasAttentionAcknowledged('sess-claude-review')).toBe(true)
  })
})
