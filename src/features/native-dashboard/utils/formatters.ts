import type {
  AgentSource,
  AttentionKind,
  IoQuality,
  MetricAvailability,
  QuotaSnapshotView,
  SoundEvent,
} from '../../../native/contracts'

const AGENT_LABELS: Record<AgentSource, string> = {
  cursor: 'Cursor',
  claudeCode: 'Claude Code',
  codex: 'Codex',
  gemini: 'Gemini CLI',
  antigravityCli: 'Antigravity CLI',
  copilotCli: 'GitHub Copilot CLI',
  qwen: 'Qwen Code',
  generic: 'Generic',
  unknown: 'Unknown',
}

const ROUTING_AGENT_LABELS: Record<string, string> = {
  'antigravity-cli': 'Antigravity CLI',
  'copilot-cli': 'GitHub Copilot CLI',
  'qwen-cli': 'Qwen Code',
}

const ATTENTION_LABELS: Record<AttentionKind, string> = {
  none: 'None',
  approval: 'Approval',
  question: 'Question',
  permission: 'Permission',
  error: 'Error',
}

const SOUND_EVENT_LABELS: Record<SoundEvent, string> = {
  approval: 'Approval',
  question: 'Question',
  completed: 'Completed',
  failed: 'Failed',
  notification: 'Notification',
}

export function agentLabel(source: AgentSource): string {
  return AGENT_LABELS[source]
}

/** Labels sound-routing agent keys, including catalog wire IDs beyond `AgentSource`. */
export function routingAgentLabel(source: string): string {
  if (source in AGENT_LABELS) {
    return AGENT_LABELS[source as AgentSource]
  }
  return ROUTING_AGENT_LABELS[source] ?? source
}

export function attentionLabel(kind: AttentionKind): string {
  return ATTENTION_LABELS[kind]
}

export function soundEventLabel(event: SoundEvent): string {
  return SOUND_EVENT_LABELS[event]
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return '—'
  }
  if (bytes < 1024) {
    return `${bytes} B`
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

export function formatBytesPerSec(bytes: number): string {
  return `${formatBytes(bytes)}/s`
}

export function formatPercent(value: number, digits = 1): string {
  if (!Number.isFinite(value)) {
    return '—'
  }
  return `${value.toFixed(digits)}%`
}

export function formatDurationMs(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) {
    return '—'
  }
  const totalSeconds = Math.floor(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) {
    return `${hours}h ${minutes}m`
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

const QUOTA_STALE_AFTER_MS = 10 * 60_000

export interface QuotaObservedSummary {
  observedAtMs: number
  freshness: 'fresh' | 'stale'
}

export function quotaObservedSummary(
  quotas: QuotaSnapshotView[],
  nowMs: number,
): QuotaObservedSummary | undefined {
  const observedAtMs = quotas.reduce<number | undefined>((latest, quota) => {
    const atMs = quota.observedAtMs
    if (typeof atMs !== 'number' || !Number.isFinite(atMs)) {
      return latest
    }
    return latest === undefined ? atMs : Math.max(latest, atMs)
  }, undefined)
  if (observedAtMs === undefined) {
    return undefined
  }

  const freshnessValues = quotas
    .filter(
      (quota) =>
        typeof quota.observedAtMs === 'number' &&
        Number.isFinite(quota.observedAtMs) &&
        typeof quota.freshness === 'string',
    )
    .map((quota) => quota.freshness)
  const freshness =
    freshnessValues.includes('stale')
      ? 'stale'
      : freshnessValues.includes('fresh')
        ? 'fresh'
        : nowMs - observedAtMs > QUOTA_STALE_AFTER_MS
          ? 'stale'
          : 'fresh'

  return { observedAtMs, freshness }
}

export function formatRelativeTime(atMs: number, nowMs: number): string {
  const delta = Math.max(0, nowMs - atMs)
  if (delta < 60_000) {
    return 'just now'
  }
  if (delta < 3_600_000) {
    return `${Math.floor(delta / 60_000)}m ago`
  }
  if (delta < 86_400_000) {
    return `${Math.floor(delta / 3_600_000)}h ago`
  }
  return `${Math.floor(delta / 86_400_000)}d ago`
}

export function metricAvailabilityLabel(value: MetricAvailability): string {
  switch (value) {
    case 'available':
      return 'Available'
    case 'warmingUp':
      return 'Warming up'
    case 'unavailable':
      return 'Unavailable'
  }
}

export function ioQualityLabel(value: IoQuality): string {
  switch (value) {
    case 'disk':
      return 'Disk I/O'
    case 'allIo':
      return 'All I/O (Windows)'
    case 'partial':
      return 'Partial I/O'
    case 'unavailable':
      return 'I/O unavailable'
  }
}

export function historyRangeLabel(range: '15m' | '1h' | '24h'): string {
  switch (range) {
    case '15m':
      return '15 minutes'
    case '1h':
      return '1 hour'
    case '24h':
      return '24 hours'
  }
}

export function isModifierPressed(event: KeyboardEvent | React.KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey
}
