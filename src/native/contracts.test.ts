import { describe, expect, it } from 'vitest'
import {
  attributionQualityLabel,
  type HealthProbeResult,
  mapProbesToUserStatus,
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

    const helperMissing: HealthProbeResult[] = [
      { axis: 'installation', outcome: 'ok' },
      { axis: 'trust', outcome: 'ok' },
      { axis: 'helper', outcome: 'fail', failureKind: 'helperPathMissing' },
      { axis: 'traffic', outcome: 'fail', failureKind: 'noTraffic' },
    ]
    expect(mapProbesToUserStatus(helperMissing)).toBe('helperMissing')

    const hooksMisconfigured: HealthProbeResult[] = [
      { axis: 'installation', outcome: 'ok' },
      { axis: 'trust', outcome: 'ok' },
      { axis: 'helper', outcome: 'fail', failureKind: 'hooksMisconfigured' },
      { axis: 'traffic', outcome: 'fail', failureKind: 'noTraffic' },
    ]
    expect(mapProbesToUserStatus(hooksMisconfigured)).toBe('hooksMisconfigured')
  })
})
