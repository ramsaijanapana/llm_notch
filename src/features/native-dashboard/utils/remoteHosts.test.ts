import { describe, expect, it } from 'vitest'
import { mockRemoteHosts } from '../fixtures/testFixtures'
import { applyRemoteConnectionStatus } from './remoteHosts'

describe('applyRemoteConnectionStatus', () => {
  it('updates connection fields for a matching host', () => {
    const updated = applyRemoteConnectionStatus(mockRemoteHosts, {
      hostId: 'dev-box',
      availability: 'available',
      connectionState: 'connecting',
      message: 'Opening SSH session',
    })

    expect(updated).toHaveLength(1)
    expect(updated[0]).toMatchObject({
      availability: 'available',
      connectionState: 'connecting',
      message: 'Opening SSH session',
    })
  })

  it('records lastConnectedAtMs when streaming begins', () => {
    const nowMs = 1_700_000_123_000
    const updated = applyRemoteConnectionStatus(
      mockRemoteHosts,
      {
        hostId: 'dev-box',
        availability: 'available',
        connectionState: 'streaming',
        message: null,
      },
      nowMs,
    )

    expect(updated[0]?.lastConnectedAtMs).toBe(nowMs)
  })

  it('leaves hosts unchanged when the event targets an unknown host', () => {
    const updated = applyRemoteConnectionStatus(mockRemoteHosts, {
      hostId: 'missing-host',
      availability: 'available',
      connectionState: 'failed',
      message: 'Host not found',
    })

    expect(updated).toEqual(mockRemoteHosts)
  })
})
