import type {
  AgentSource,
  AttentionKind,
  AttributionQuality,
  IoQuality,
  MetricAvailability,
  SessionStatus,
} from '../../../native/contracts'
import { attributionQualityLabel } from '../../../native/contracts'

export function formatAgentSource(source: AgentSource): string {
  switch (source) {
    case 'cursor':
      return 'Cursor'
    case 'claudeCode':
      return 'Claude Code'
    case 'codex':
      return 'Codex'
    case 'gemini':
      return 'Gemini CLI'
    case 'antigravityCli':
    case 'agy':
      return 'Antigravity CLI'
    case 'copilotCli':
      return 'GitHub Copilot CLI'
    case 'qwen':
      return 'Qwen Code'
    case 'generic':
      return 'Generic'
    case 'unknown':
      return 'Unknown'
  }
}

export function formatSessionStatus(status: SessionStatus): string {
  switch (status) {
    case 'starting':
      return 'Starting'
    case 'running':
      return 'Running'
    case 'waiting':
      return 'Waiting'
    case 'paused':
      return 'Paused'
    case 'completed':
      return 'Completed'
    case 'failed':
      return 'Failed'
    case 'stale':
      return 'Stale'
  }
}

export function formatAttentionKind(kind: AttentionKind): string {
  switch (kind) {
    case 'approval':
      return 'Approval needed'
    case 'question':
      return 'Question pending'
    case 'permission':
      return 'Permission needed'
    case 'error':
      return 'Error'
    case 'none':
      return 'No attention'
  }
}

export function formatAttributionQuality(quality: AttributionQuality): string {
  return attributionQualityLabel(quality)
}

export function formatIoQuality(quality: IoQuality): string {
  switch (quality) {
    case 'disk':
      return 'Disk'
    case 'allIo':
      return 'All I/O'
    case 'partial':
      return 'Partial'
    case 'unavailable':
      return 'Unavailable'
  }
}

export function formatMetricAvailability(availability: MetricAvailability): string {
  switch (availability) {
    case 'available':
      return 'Available'
    case 'warmingUp':
      return 'Warming up'
    case 'unavailable':
      return 'Unavailable'
  }
}

export function formatCpuPercent(
  value: number | undefined,
  availability: MetricAvailability,
): string {
  if (availability === 'warmingUp') {
    return '…'
  }
  if (availability === 'unavailable' || value === undefined || Number.isNaN(value)) {
    return '—'
  }
  return `${Math.round(value)}%`
}

export function formatBytes(bytes: number | undefined): string {
  if (bytes === undefined || Number.isNaN(bytes)) {
    return '—'
  }

  const abs = Math.abs(bytes)
  if (abs >= 1024 ** 3) {
    return `${(bytes / 1024 ** 3).toFixed(1)} GB`
  }
  if (abs >= 1024 ** 2) {
    return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  }
  if (abs >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }
  return `${Math.round(bytes)} B`
}

export function formatThroughput(bytesPerSec: number | undefined): string {
  if (bytesPerSec === undefined || Number.isNaN(bytesPerSec)) {
    return '—'
  }
  return `${formatBytes(bytesPerSec)}/s`
}

export function formatRuntime(runtimeMs: number | undefined): string {
  if (runtimeMs === undefined || Number.isNaN(runtimeMs)) {
    return '—'
  }

  const totalSeconds = Math.max(0, Math.floor(runtimeMs / 1000))
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

export function summarizeSparkline(values: readonly number[]): string {
  if (values.length === 0) {
    return 'No CPU samples in the last 30 seconds'
  }

  const min = Math.min(...values)
  const max = Math.max(...values)
  const latest = values.at(-1) ?? 0
  return `CPU trend over last 30 seconds. Latest ${Math.round(latest)} percent. Range ${Math.round(min)} to ${Math.round(max)} percent.`
}
