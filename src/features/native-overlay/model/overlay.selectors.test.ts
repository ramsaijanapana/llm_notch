import { describe, expect, it } from 'vitest'
import {
  buildSparklinePoints,
  countAttentionSessions,
  deriveHealthBeaconTone,
  getCombinedCpuReading,
  getCompactStatusHint,
  getConnectionBanner,
  resolveConnectionBannerText,
  selectAttentionSessions,
  selectCompactDots,
  sortSessionsForPeek,
  sparklinePolyline,
} from './overlay.selectors'
import { createCpuHistory, createSession, createSnapshot } from './testFixtures'

describe('overlay.selectors', () => {
  it('counts attention sessions and sorts attention first in peek', () => {
    const snapshot = createSnapshot()
    expect(countAttentionSessions(snapshot.sessions)).toBe(2)

    const ordered = sortSessionsForPeek(snapshot.sessions)
    expect(ordered[0]?.attention).not.toBe('none')
    expect(ordered[1]?.attention).not.toBe('none')
  })

  it('limits compact dots to six with overflow count', () => {
    const sessions = Array.from({ length: 8 }, (_, index) =>
      createSession({ id: `session-${index}`, label: `Session ${index}` }),
    )

    const selection = selectCompactDots(sessions)
    expect(selection.visible).toHaveLength(6)
    expect(selection.overflowCount).toBe(2)
  })

  it('selects only attention sessions for the peek banner section', () => {
    const snapshot = createSnapshot()
    const attention = selectAttentionSessions(snapshot.sessions)
    expect(attention).toHaveLength(2)
    expect(attention.every((session) => session.attention !== 'none')).toBe(true)
  })

  it('derives health beacon tones from connection and attention', () => {
    expect(deriveHealthBeaconTone('live', 0)).toBe('healthy')
    expect(deriveHealthBeaconTone('live', 2)).toBe('attention')
    expect(deriveHealthBeaconTone('live', 0, 1)).toBe('degraded')
    expect(deriveHealthBeaconTone('warmingUp', 0)).toBe('degraded')
    expect(deriveHealthBeaconTone('ipcError', 0)).toBe('error')
  })

  it('returns connection banners for empty and error states', () => {
    expect(getConnectionBanner('empty')).toMatch(/No active agent sessions/)
    expect(getConnectionBanner('ipcError')).toMatch(/Connection to agent core lost/)
    expect(getConnectionBanner('coreError')).toMatch(/Agent core error/)
    expect(getConnectionBanner('stale')).toMatch(/Resyncing stream/)
    expect(getConnectionBanner('warmingUp')).toMatch(/warming up/)
    expect(getConnectionBanner('metricsUnavailable')).toMatch(/Metrics unavailable/)
    expect(getConnectionBanner('live')).toBeNull()
  })

  it('prefers custom resync and error messages in banner resolution', () => {
    expect(resolveConnectionBannerText('stale', { staleMessage: 'Sequence gap at frame 42' })).toBe(
      'Sequence gap at frame 42',
    )
    expect(resolveConnectionBannerText('ipcError', { errorMessage: 'Stream channel closed' })).toBe(
      'Stream channel closed',
    )
  })

  it('shows compact status hints when disconnected even with cached sessions', () => {
    const snapshot = createSnapshot()
    expect(
      getCompactStatusHint('ipcError', snapshot.sessions.length, {
        errorMessage: 'Stream channel closed',
      }),
    ).toBe('Stream channel closed')
    expect(
      getCompactStatusHint('stale', snapshot.sessions.length, {
        staleMessage: 'Resyncing native stream after sequence gap.',
      }),
    ).toBe('Resyncing native stream after sequence gap.')
    expect(getCompactStatusHint('live', snapshot.sessions.length)).toBeNull()
    expect(getCompactStatusHint('empty', 0)).toMatch(/No active agent sessions/)
  })

  it('reads combined CPU from aggregate metrics', () => {
    const snapshot = createSnapshot()
    expect(getCombinedCpuReading(snapshot)).toEqual({
      value: 88,
      availability: 'available',
    })
    expect(getCombinedCpuReading(undefined)).toEqual({
      value: undefined,
      availability: 'unavailable',
    })
  })

  it('builds sparkline points within the 30 second window', () => {
    const nowMs = 1_700_000_000_000
    const history = createCpuHistory(nowMs)
    const points = buildSparklinePoints(history, nowMs)
    expect(points.length).toBeGreaterThan(0)
    expect(sparklinePolyline(points)).toMatch(/\d/)
  })
})
