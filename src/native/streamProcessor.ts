import type { MetricsFrame, StreamFrame, StreamPayload } from './contracts.ts'

export type StreamAcceptance =
  | { kind: 'accept'; nextSequence: number }
  | { kind: 'duplicate' }
  | { kind: 'gap'; expected: number; received: number }

export function evaluateStreamSequence(
  frame: StreamFrame,
  lastSequence: number | null,
): StreamAcceptance {
  if (lastSequence === null) {
    return { kind: 'accept', nextSequence: frame.sequence }
  }

  if (frame.sequence <= lastSequence) {
    return { kind: 'duplicate' }
  }

  if (frame.sequence === lastSequence + 1) {
    return { kind: 'accept', nextSequence: frame.sequence }
  }

  return { kind: 'gap', expected: lastSequence + 1, received: frame.sequence }
}

export function isMetricsPayload(
  payload: StreamPayload,
): payload is Extract<StreamPayload, { type: 'metrics' }> {
  return payload.type === 'metrics'
}

/**
 * Latest-wins coalescing for high-frequency metrics frames.
 * Non-metric frames are returned unchanged.
 */
export function coalesceStreamFrames(frames: StreamFrame[]): StreamFrame[] {
  if (frames.length <= 1) {
    return frames
  }

  const coalesced: StreamFrame[] = []
  let pendingMetrics: StreamFrame | null = null

  const flushMetrics = () => {
    if (pendingMetrics) {
      coalesced.push(pendingMetrics)
      pendingMetrics = null
    }
  }

  for (const frame of frames) {
    if (isMetricsPayload(frame.payload)) {
      pendingMetrics = frame
      continue
    }

    flushMetrics()
    coalesced.push(frame)
  }

  flushMetrics()
  return coalesced
}

export function mergeMetricsFrame(
  current: MetricsFrame | null,
  incoming: MetricsFrame,
): MetricsFrame {
  return incoming ?? current
}
