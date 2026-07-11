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
export const MAX_PLAN_ID_LEN = 128 as const
export const MAX_CONNECTOR_DISPLAY_PATH_LEN = 512 as const
export const MAX_CONNECTOR_DIFF_LEN = 65_536 as const
export const MAX_CONNECTOR_PLAN_FILES = 16 as const
export const CONNECTOR_PLAN_TTL_MS = 300_000 as const
export const MAX_DECISION_SUMMARY_LEN = 512 as const
export const MAX_DECISION_ANSWER_LEN = 4_096 as const

export const DECISION_FAIL_OPEN_TIMEOUT_MS = 2_000 as const
export const DECISION_HOOK_NEUTRAL_OUTPUT = '{}' as const
export const DECISION_HOOK_FAIL_OPEN_EXIT_CODE = 0 as const

export const MIGRATION_REGISTRY_VERSION = 1 as const

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

export type ContextOpenTier = 'none' | 'appActivate' | 'windowFocus' | 'exactPane'

export interface AdapterObservationPaths {
  lifecycleEvents: boolean
  toolEvents: boolean
  attentionEvents: boolean
}

export interface AdapterResponsePaths {
  decisions: boolean
  questions: boolean
  contextOpenTier: ContextOpenTier
}

export interface AdapterCapabilities {
  source: AgentSource
  events: boolean
  attention: AttentionCapability
  decisionResponse: boolean
  contextOpen: boolean
  processAttribution: AttributionQuality
  contextOpenTier?: ContextOpenTier
  observeLifecycle?: boolean
  observeTools?: boolean
  respondDecisions?: boolean
  respondQuestions?: boolean
  failOpenHooks?: boolean
  requiresExternalTrust?: boolean
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

// --- Contract freeze v2 (connector, health, decision, purge, migration) ---

export type HealthProbeAxis = 'installation' | 'trust' | 'traffic' | 'helper'

export type HealthProbeOutcome = 'ok' | 'warn' | 'fail'

export type HealthProbeFailureKind =
  | 'agentNotFound'
  | 'notInstalled'
  | 'trustRequired'
  | 'helperUnavailable'
  | 'noTraffic'
  | 'configDrift'
  | 'internalError'

export interface HealthProbeResult {
  axis: HealthProbeAxis
  outcome: HealthProbeOutcome
  failureKind?: HealthProbeFailureKind
  detail?: string
}

export type ConnectorUserStatus =
  | 'notFound'
  | 'notInstalled'
  | 'actionNeeded'
  | 'waitingFirstEvent'
  | 'connected'
  | 'driftDetected'
  | 'error'

export type ConnectorScope = 'user' | 'project'

export type ExternalTrustActionKind = 'codexHooksReview' | 'other'

export interface ExternalTrustAction {
  kind: ExternalTrustActionKind
  instructions: string
}

export interface ConnectorFilePreview {
  /** Display-only redacted path; canonical identity is backend-only. */
  displayPath: string
  baselineSha256: string
  diffText: string
  foreignEntriesPreserved: string[]
  isNewFile: boolean
}

export interface ConnectorPlanPreview {
  planId: string
  source: AgentSource
  scope: ConnectorScope
  expiresAtMs: number
  summary: string
  files: ConnectorFilePreview[]
  externalTrustActions: ExternalTrustAction[]
  backupDisplayHint?: string
}

export type ConnectorFileOutcome = 'applied' | 'skipped' | 'failed'

export type ConnectorErrorCode =
  | 'planExpired'
  | 'planNotFound'
  | 'fileChangedSincePreview'
  | 'lockContention'
  | 'pathEscapesScope'
  | 'partialApplyFailure'
  | 'activeConnectorBlocked'
  | 'rollbackHashMismatch'
  | 'internalError'

export interface ConnectorFileApplyResult {
  displayPath: string
  outcome: ConnectorFileOutcome
  backupJournalId?: string
  appliedHash?: string
  errorCode?: ConnectorErrorCode
  message?: string
}

export interface ConnectorApplyResult {
  planId: string
  source: AgentSource
  fileResults: ConnectorFileApplyResult[]
  capabilities: AdapterCapabilities
}

export interface ConnectorApplyError {
  code: ConnectorErrorCode
  message: string
  expectedSha256?: string
  actualSha256?: string
  partialResults?: ConnectorFileApplyResult[]
}

export type BackupJournalOperation = 'create' | 'restore' | 'recompute'

export interface BackupJournalEntry {
  id: string
  planId?: string
  source: AgentSource
  displayPath: string
  backupDisplayPath: string
  contentSha256: string
  appliedHash?: string
  operation: BackupJournalOperation
  recordedAtMs: number
}

export interface ConnectorJournalEntry {
  id: string
  planId: string
  source: AgentSource
  scope: ConnectorScope
  startedAtMs: number
  completedAtMs?: number
  fileResults: ConnectorFileApplyResult[]
  rollbackAvailable: boolean
}

export interface ConnectorHealthEntry {
  source: AgentSource
  status: ConnectorUserStatus
  probes: HealthProbeResult[]
  capabilities: AdapterCapabilities
  detail?: string
}

export interface ConnectorHealthReport {
  checkedAtMs: number
  adapters: ConnectorHealthEntry[]
}

export type DecisionKind = 'approval' | 'permission' | 'question'

export type DecisionResponseAction = 'allow' | 'deny'

export type DecisionResponse =
  | { type: 'action'; action: DecisionResponseAction }
  | { type: 'answer'; text: string }

export type DecisionDeliveryState =
  | 'pending'
  | 'delivered'
  | 'effectObserved'
  | 'expired'
  | 'failed'

export interface DecisionRequest {
  id: string
  sessionId: string
  source: AgentSource
  kind: DecisionKind
  summary: string
  hasActionablePayload: boolean
  createdAtMs: number
  expiresAtMs?: number
}

export interface DecisionResponseRecord {
  requestId: string
  response: DecisionResponse
  respondedAtMs: number
  deliveryState: DecisionDeliveryState
  deliveryDetail?: string
}

export interface PurgeScope {
  history?: boolean
  sessionEvents?: boolean
  connectorJournal?: boolean
  /** Explicit opt-in; backups kept by default. */
  includeBackups?: boolean
}

export interface PurgeResult {
  historyRowsRemoved: number
  eventsRemoved: number
  backupsRemoved: number
  activeConnectorsDisconnected: number
}

export type MigrationLane = 'connectors' | 'decisions' | 'metrics' | 'platform'

export interface MigrationRecord {
  lane: MigrationLane
  version: number
  appliedAtMs: number
  checksum?: string
}

export interface MigrationRegistry {
  registryVersion: number
  records: MigrationRecord[]
}

/** Maps wire `AttributionQuality.unknown` to user-facing "Not attributed". */
export function attributionQualityLabel(quality: AttributionQuality): string {
  if (quality === 'unknown') {
    return 'Not attributed'
  }
  switch (quality) {
    case 'exact':
      return 'Exact'
    case 'shared':
      return 'Shared'
    case 'heuristic':
      return 'Heuristic'
    default:
      return 'Not attributed'
  }
}

/** Deterministic user-facing status from orthogonal probe results. */
export function mapProbesToUserStatus(probes: HealthProbeResult[]): ConnectorUserStatus {
  const probe = (axis: HealthProbeAxis) => probes.find((entry) => entry.axis === axis)

  const installation = probe('installation')
  if (installation?.outcome === 'fail') {
    return installation.failureKind === 'agentNotFound' ? 'notFound' : 'notInstalled'
  }
  if (installation?.outcome === 'warn') {
    return 'driftDetected'
  }

  const trust = probe('trust')
  if (trust?.outcome === 'fail' || trust?.outcome === 'warn') {
    return 'actionNeeded'
  }

  if (probe('helper')?.outcome === 'fail') {
    return 'error'
  }

  const traffic = probe('traffic')
  if (traffic?.outcome === 'fail' || traffic?.outcome === 'warn') {
    return 'waitingFirstEvent'
  }

  if (probes.some((entry) => entry.outcome === 'warn')) {
    return 'driftDetected'
  }

  if (probes.length > 0 && probes.every((entry) => entry.outcome === 'ok')) {
    return 'connected'
  }

  return 'error'
}
