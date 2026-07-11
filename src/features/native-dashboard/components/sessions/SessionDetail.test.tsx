import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import type { AdapterCapabilities, AgentSession, MetricSample } from '../../../../native/contracts'
import { SessionDetail } from './SessionDetail'

const adapter: AdapterCapabilities = {
  source: 'generic',
  events: true,
  attention: 'full',
  decisionResponse: false,
  contextOpen: false,
  processAttribution: 'exact',
}

const metric: MetricSample = {
  atMs: 1,
  cpuCorePercent: 12,
  cpuHostPercent: 3,
  rssBytes: 1024,
  runtimeMs: 1000,
  processCount: 1,
  readBytesPerSec: 0,
  writeBytesPerSec: 0,
  quality: {
    attribution: 'exact',
    cpu: 'available',
    io: 'disk',
  },
}

const session: AgentSession = {
  id: 'session-live',
  source: 'generic',
  externalSessionId: 'external-live',
  label: 'Live metrics',
  status: 'running',
  attention: 'none',
  startedAtMs: 1,
  lastEventAtMs: 1,
  latestMetric: metric,
}

describe('SessionDetail', () => {
  afterEach(() => cleanup())

  it('updates and clears live session metrics', () => {
    const { rerender } = render(<SessionDetail session={session} adapters={[adapter]} />)
    expect(screen.getByText('12.0%')).toBeInTheDocument()

    rerender(
      <SessionDetail
        session={{ ...session, latestMetric: { ...metric, cpuCorePercent: 77 } }}
        adapters={[adapter]}
      />,
    )
    expect(screen.getByText('77.0%')).toBeInTheDocument()

    const { latestMetric: _latestMetric, ...withoutMetric } = session
    rerender(<SessionDetail session={withoutMetric} adapters={[adapter]} />)
    expect(screen.getByText(/metrics unavailable for this session/i)).toBeInTheDocument()
  })
})
