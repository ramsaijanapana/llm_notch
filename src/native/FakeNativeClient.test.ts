import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFakeNativeClient } from './FakeNativeClient.ts'

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

  it('reports integration health for preview adapters', async () => {
    const client = createFakeNativeClient()
    const health = await client.getIntegrationHealth()

    expect(health.adapters).toHaveLength(3)
    expect(['healthy', 'degraded', 'unavailable']).toContain(health.overall)
  })

  it('acknowledges local attention without simulating vendor approvals', async () => {
    const client = createFakeNativeClient()
    await client.acknowledgeLocalAttention('sess-claude-review')

    expect(client.wasAttentionAcknowledged('sess-claude-review')).toBe(true)
  })
})
