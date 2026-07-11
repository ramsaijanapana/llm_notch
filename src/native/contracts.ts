/**
 * Frozen wire contracts mirrored from `crates/notch-protocol`.
 *
 * Owner: Stage 0 foundation agent. Keep names and camelCase fields aligned with Rust DTOs.
 */

export const PROTOCOL_VERSION = 1 as const

export const MAX_STREAM_FRAME_BYTES = 65_536 as const
export const MAX_SESSION_ID_LEN = 64 as const
export const MAX_EXTERNAL_SESSION_ID_LEN = 256 as const
export const MAX_SESSION_LABEL_LEN = 256 as const
export const MAX_WORKSPACE_LABEL_LEN = 256 as const
export const MAX_EVENT_SUMMARY_LEN = 512 as const
export const MAX_TOOL_NAME_LEN = 128 as const
export const MAX_METRIC_REASON_LEN = 512 as const
export const MAX_RESYNC_REASON_LEN = 512 as const
export const STREAM_HEARTBEAT_INTERVAL_MS = 5_000 as const
export const MAX_SNAPSHOT_SESSIONS = 128 as const

export type AgentSource = 'cursor' | 'claudeCode' | 'codex' | 'generic' | 'unknown'

export type SessionStatus =
  | 'starting'
  | 'running'
  | 'waiting'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'stale'

export type AttentionKind = 'none' | 'approval' | 'question' | 'permission' | 'error'

export type AttributionQuality = 'exact' | 'shared' | 'heuristic' | 'unknown'

export type MetricAvailability = 'available' | 'warmingUp' | 'unavailable'

export type IoQuality = 'disk' | 'allIo' | 'partial' | 'unavailable'

export type SessionEventKind = 'lifecycle' | 'tool' | 'attention' | 'status'

export type EventLevel = 'debug' | 'info' | 'warning' | 'error'

export type AttentionCapability = 'full' | 'partial' | 'none'

export interface ProcessIdentity {
  pid: number
  startedAtMs: number
}

export interface MetricQuality {
  attribution: AttributionQuality
  cpu: MetricAvailability
  io: IoQuality
  reason?: string
}

export interface MetricSample {
  atMs: number
  cpuCorePercent: number
  cpuHostPercent: number
  rssBytes: number
  runtimeMs: number
  processCount: number
  readBytesPerSec: number
  writeBytesPerSec: number
  quality: MetricQuality
}

export interface HostMetricSample {
  atMs: number
  cpuHostPercent: number
  usedMemoryBytes: number
  totalMemoryBytes: number
  visibleProcessCount: number
  diskReadBytesPerSec: number
  diskWriteBytesPerSec: number
}

export interface AgentAggregate {
  atMs: number
  cpuCorePercent: number
  cpuHostPercent: number
  rssBytes: number
  runtimeMs: number
  processCount: number
  readBytesPerSec: number
  writeBytesPerSec: number
  quality: MetricQuality
  activeSessions: number
  attentionSessions: number
}

export interface AgentSession {
  id: string
  source: AgentSource
  externalSessionId: string
  label: string
  workspaceLabel?: string
  status: SessionStatus
  attention: AttentionKind
  startedAtMs: number
  lastEventAtMs: number
  endedAtMs?: number
  processRoot?: ProcessIdentity
  latestMetric?: MetricSample
}

export interface SessionEvent {
  id: string
  sessionId: string
  sequence: number
  occurredAtMs: number
  kind: SessionEventKind
  level: EventLevel
  summary: string
  toolName?: string
}

export interface AdapterCapabilities {
  source: AgentSource
  events: boolean
  attention: AttentionCapability
  decisionResponse: boolean
  contextOpen: boolean
  processAttribution: AttributionQuality
}

export interface PublicSettings {
  overlayEnabled: boolean
  autostartEnabled: boolean
  reducedMotion: boolean
  samplingIntervalMs: number
  selectedDisplay?: string
  showOverFullscreen: boolean
  historyRetentionHours: number
}

export interface MetricsFrame {
  host: HostMetricSample
  aggregate: AgentAggregate
  agents: Record<string, MetricSample>
}

export interface AppSnapshot {
  protocolVersion: number
  capturedAtMs: number
  host?: HostMetricSample
  aggregate?: AgentAggregate
  sessions: AgentSession[]
  settings: PublicSettings
  adapters: AdapterCapabilities[]
}

export type StreamPayload =
  | { type: 'snapshot'; snapshot: AppSnapshot }
  | { type: 'sessionUpsert'; session: AgentSession }
  | { type: 'sessionRemove'; sessionId: string }
  | { type: 'sessionEvent'; event: SessionEvent }
  | { type: 'metrics'; metrics: MetricsFrame }
  | { type: 'settingsChanged'; settings: PublicSettings }
  | { type: 'integrationChanged'; integration: AdapterCapabilities }
  | { type: 'heartbeat' }
  | { type: 'resyncRequired'; reason: string }

export interface StreamFrame {
  sequence: number
  emittedAtMs: number
  payload: StreamPayload
}
