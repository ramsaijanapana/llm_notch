import type {
  AgentSession,
  AppSnapshot,
  AttributionQuality,
  IoQuality,
  MetricAvailability,
} from '../../../native/contracts'
import type { HealthBeaconTone, OverlayConnectionState, OverlayCpuSample } from '../types'

export const MAX_COMPACT_DOTS = 6
export const SPARKLINE_WINDOW_MS = 30_000

export type SessionDotTone =
  | 'attention'
  | 'running'
  | 'waiting'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'stale'
  | 'starting'

export interface CompactDotSelection {
  visible: AgentSession[]
  overflowCount: number
}

export interface SparklinePoint {
  x: number
  y: number
}

export interface CombinedCpuReading {
  value: number | undefined
  availability: MetricAvailability
}

export interface FooterMetrics {
  cpuCorePercent: number | undefined
  cpuAvailability: MetricAvailability
  rssBytes: number | undefined
  readBytesPerSec: number | undefined
  writeBytesPerSec: number | undefined
  processCount: number | undefined
  attributionLabel: AttributionQuality | undefined
  ioLabel: IoQuality | undefined
}

export function isAttentionSession(session: AgentSession): boolean {
  return session.attention !== 'none'
}

export function countAttentionSessions(sessions: readonly AgentSession[]): number {
  return sessions.filter(isAttentionSession).length
}

export function sortSessionsForPeek(sessions: readonly AgentSession[]): AgentSession[] {
  return [...sessions].sort((left, right) => {
    const leftAttention = isAttentionSession(left) ? 1 : 0
    const rightAttention = isAttentionSession(right) ? 1 : 0
    if (leftAttention !== rightAttention) {
      return rightAttention - leftAttention
    }
    return right.lastEventAtMs - left.lastEventAtMs
  })
}

export function selectCompactDots(sessions: readonly AgentSession[]): CompactDotSelection {
  const sorted = sortSessionsForPeek(sessions)
  return {
    visible: sorted.slice(0, MAX_COMPACT_DOTS),
    overflowCount: Math.max(0, sorted.length - MAX_COMPACT_DOTS),
  }
}

export function getSessionDotTone(session: AgentSession): SessionDotTone {
  if (isAttentionSession(session)) {
    return 'attention'
  }

  switch (session.status) {
    case 'running':
      return 'running'
    case 'waiting':
      return 'waiting'
    case 'paused':
      return 'paused'
    case 'completed':
      return 'completed'
    case 'failed':
      return 'failed'
    case 'stale':
      return 'stale'
    case 'starting':
      return 'starting'
  }
}

export function deriveHealthBeaconTone(
  connectionState: OverlayConnectionState,
  attentionCount: number,
  resourceAlertCount = 0,
): HealthBeaconTone {
  if (connectionState === 'ipcError' || connectionState === 'coreError') {
    return 'error'
  }
  if (attentionCount > 0) {
    return 'attention'
  }
  if (resourceAlertCount > 0) {
    return 'degraded'
  }
  if (
    connectionState === 'stale' ||
    connectionState === 'warmingUp' ||
    connectionState === 'metricsUnavailable'
  ) {
    return 'degraded'
  }
  return 'healthy'
}

export function getConnectionBanner(
  connectionState: OverlayConnectionState,
  emptyMessage?: string | null | undefined,
): string | null {
  switch (connectionState) {
    case 'empty':
      return emptyMessage ?? 'No active agent sessions'
    case 'ipcError':
      return 'Connection to agent core lost'
    case 'coreError':
      return 'Agent core error'
    case 'stale':
      return 'Resyncing stream'
    case 'warmingUp':
      return 'Metrics warming up'
    case 'metricsUnavailable':
      return 'Metrics unavailable'
    case 'live':
      return null
  }
}

export function resolveConnectionBannerText(
  connectionState: OverlayConnectionState,
  options?: {
    emptyMessage?: string | null | undefined
    staleMessage?: string | undefined
    errorMessage?: string | undefined
  },
): string | null {
  const { emptyMessage, staleMessage, errorMessage } = options ?? {}
  const banner = getConnectionBanner(connectionState, emptyMessage)
  if (connectionState === 'stale' && staleMessage) {
    return staleMessage
  }
  if ((connectionState === 'ipcError' || connectionState === 'coreError') && errorMessage) {
    return errorMessage
  }
  return banner
}

export type CompactHintTone = 'error' | 'warning' | 'muted'

export function getCompactHintTone(connectionState: OverlayConnectionState): CompactHintTone {
  if (connectionState === 'ipcError' || connectionState === 'coreError') {
    return 'error'
  }
  if (connectionState === 'stale') {
    return 'warning'
  }
  return 'muted'
}

export function getCompactStatusHint(
  connectionState: OverlayConnectionState,
  sessionCount: number,
  options?: {
    emptyMessage?: string | null | undefined
    staleMessage?: string | undefined
    errorMessage?: string | undefined
  },
): string | null {
  const text = resolveConnectionBannerText(connectionState, options)
  if (!text) {
    return null
  }
  if (sessionCount === 0) {
    return text
  }
  if (
    connectionState === 'ipcError' ||
    connectionState === 'coreError' ||
    connectionState === 'stale'
  ) {
    return text
  }
  return null
}

export function getCombinedCpuReading(snapshot: AppSnapshot | undefined): CombinedCpuReading {
  const aggregate = snapshot?.aggregate
  if (!aggregate) {
    return { value: undefined, availability: 'unavailable' }
  }

  return {
    value: aggregate.cpuCorePercent,
    availability: aggregate.quality.cpu,
  }
}

export function getFooterMetrics(snapshot: AppSnapshot | undefined): FooterMetrics {
  const aggregate = snapshot?.aggregate
  if (!aggregate) {
    return {
      cpuCorePercent: undefined,
      cpuAvailability: 'unavailable',
      rssBytes: undefined,
      readBytesPerSec: undefined,
      writeBytesPerSec: undefined,
      processCount: undefined,
      attributionLabel: undefined,
      ioLabel: undefined,
    }
  }

  return {
    cpuCorePercent: aggregate.cpuCorePercent,
    cpuAvailability: aggregate.quality.cpu,
    rssBytes: aggregate.rssBytes,
    readBytesPerSec: aggregate.readBytesPerSec,
    writeBytesPerSec: aggregate.writeBytesPerSec,
    processCount: aggregate.processCount,
    attributionLabel: aggregate.quality.attribution,
    ioLabel: aggregate.quality.io,
  }
}

export function buildSparklinePoints(
  history: readonly OverlayCpuSample[],
  nowMs: number,
  width = 56,
  height = 18,
  windowMs = SPARKLINE_WINDOW_MS,
): SparklinePoint[] {
  const windowStart = nowMs - windowMs
  const samples = history
    .filter((sample) => sample.atMs >= windowStart && sample.atMs <= nowMs)
    .sort((left, right) => left.atMs - right.atMs)

  if (samples.length === 0) {
    return []
  }

  const values = samples.map((sample) => sample.cpuCorePercent)
  const maxValue = Math.max(100, ...values)

  return samples.map((sample, index) => {
    const x = samples.length === 1 ? width / 2 : (index / (samples.length - 1)) * width
    const normalized = sample.cpuCorePercent / maxValue
    const y = height - normalized * height
    return { x, y }
  })
}

export function sparklinePolyline(points: readonly SparklinePoint[]): string {
  return points.map((point) => `${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(' ')
}

export function selectAttentionSessions(sessions: readonly AgentSession[]): AgentSession[] {
  return sortSessionsForPeek(sessions).filter(isAttentionSession)
}

export function compactAriaLabel(params: {
  attentionCount: number
  sessionCount: number
  cpuLabel: string
  connectionState: OverlayConnectionState
  emptyMessage?: string | null | undefined
}): string {
  const banner = resolveConnectionBannerText(params.connectionState, {
    emptyMessage: params.emptyMessage,
  })
  const parts = [
    'Agent overlay compact view.',
    params.attentionCount > 0
      ? `${params.attentionCount} sessions need attention.`
      : 'No sessions need attention.',
    `${params.sessionCount} tracked sessions.`,
    `Combined CPU ${params.cpuLabel}.`,
  ]

  if (banner) {
    parts.push(banner)
  }

  return parts.join(' ')
}
