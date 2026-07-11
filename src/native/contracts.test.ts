import { describe, expect, it } from 'vitest'
import {
  attributionQualityLabel,
  mapProbesToUserStatus,
  type HealthProbeResult,
} from './contracts.ts'

describe('contract freeze v2 helpers', () => {
  it('maps unknown attribution to Not attributed display label', () => {
    expect(attributionQualityLabel('unknown')).toBe('Not attributed')
    expect(attributionQualityLabel('exact')).toBe('Exact')
  })

  it('maps probe vectors to user-facing connector status', () => {
    const connected: HealthProbeResult[] = [
      { axis: 'installation', outcome: 'ok' },
      { axis: 'trust', outcome: 'ok' },
      { axis: 'traffic', outcome: 'ok' },
      { axis: 'helper', outcome: 'ok' },
    ]
    expect(mapProbesToUserStatus(connected)).toBe('connected')

    const waiting: HealthProbeResult[] = [
      { axis: 'installation', outcome: 'ok' },
      { axis: 'trust', outcome: 'ok' },
      { axis: 'traffic', outcome: 'fail', failureKind: 'noTraffic' },
      { axis: 'helper', outcome: 'ok' },
    ]
    expect(mapProbesToUserStatus(waiting)).toBe('waitingFirstEvent')
  })
})
