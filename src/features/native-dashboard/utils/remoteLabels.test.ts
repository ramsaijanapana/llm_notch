import { describe, expect, it } from 'vitest'
import {
  remoteConnectionBadgeTone,
  remoteConnectionStateLabel,
  remoteDeploymentStepLabel,
} from './remoteLabels'

describe('remoteLabels', () => {
  it('labels connection states honestly', () => {
    expect(remoteConnectionStateLabel('disconnected')).toBe('Disconnected')
    expect(remoteConnectionStateLabel('streaming')).toBe('Streaming')
    expect(remoteConnectionStateLabel({ backoff: { attempt: 2, delayMs: 1_000 } })).toBe(
      'Backoff (attempt 2)',
    )
  })

  it('maps badge tones for lifecycle states', () => {
    expect(remoteConnectionBadgeTone('streaming')).toBe('success')
    expect(remoteConnectionBadgeTone('failed')).toBe('error')
    expect(remoteConnectionBadgeTone({ backoff: { attempt: 1, delayMs: 500 } })).toBe('warning')
  })

  it('labels deployment steps from protocol shape', () => {
    expect(remoteDeploymentStepLabel({ type: 'probeTarget' })).toBe('Probe remote target')
    expect(
      remoteDeploymentStepLabel({
        type: 'activateAtomically',
        remotePath: '~/.llm-notch/bin/llm-notch-relay',
      }),
    ).toBe('Activate relay at ~/.llm-notch/bin/llm-notch-relay')
    expect(
      remoteDeploymentStepLabel({
        type: 'startStdioRelay',
        remotePath: '~/.llm-notch/bin/llm-notch-relay',
        eventSpoolDir: '~/.llm-notch',
      }),
    ).toContain('LLM_NOTCH_EVENT_SPOOL=1')
  })
})
