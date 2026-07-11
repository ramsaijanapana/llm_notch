import type {
  AdapterCapabilities,
  AppSnapshot,
  BackupJournalEntry,
  ConnectorApplyResult,
  ConnectorHealthReport,
  ConnectorPlanPreview,
  ConnectorScope,
  ConnectorUserStatus,
  DecisionRequest,
  DecisionResponse,
  DecisionResponseRecord,
  DetectedConnector,
  HealthProbeResult,
  PublicSettings,
  SessionEvent,
  StreamFrame,
} from './contracts.ts'

export type {
  BackupJournalEntry,
  ConnectorHealthEntry,
  ConnectorHealthReport,
  ConnectorPlanPreview,
  ConnectorUserStatus,
  DecisionRequest,
  DecisionResponse,
  DecisionResponseRecord,
  DetectedConnector,
  HealthProbeResult,
} from './contracts.ts'

export { mapProbesToUserStatus } from './contracts.ts'

export type NativeClientMode = 'tauri' | 'preview'

export type OverlayMode = 'collapsed' | 'peek' | 'expanded'

export interface BootstrapResult {
  snapshot: AppSnapshot
  lastSequence: number
  events: SessionEvent[]
}

export interface StreamSubscription {
  unsubscribe(): Promise<void>
}

/** @deprecated Use ConnectorUserStatus from contracts.ts */
export type IntegrationHealthStatus = ConnectorUserStatus

/** @deprecated Use ConnectorHealthEntry from contracts.ts */
export interface IntegrationHealthEntry {
  source: AdapterCapabilities['source']
  status: ConnectorUserStatus
  probes: HealthProbeResult[]
  capabilities: AdapterCapabilities
  detail?: string
}

/** @deprecated Use ConnectorHealthReport from contracts.ts */
export type IntegrationHealthReport = ConnectorHealthReport

/** @deprecated Use ConnectorPlanPreview from contracts.ts */
export type ConnectorPreview = ConnectorPlanPreview

export type NativeHistoryRange = '15m' | '1h' | '24h'

export interface NativeHistoryPoint {
  atMs: number
  cpuHostPercent: number
  cpuCorePercent: number
  rssBytes: number
}

export interface NativeAgentHistorySeries {
  sessionId: string
  points: NativeHistoryPoint[]
  actualFirstMs: number | null
  actualLastMs: number | null
  totalPoints: number
  returnedPoints: number
  downsampled: boolean
  truncated: boolean
}

export interface NativeHistorySeries {
  points: NativeHistoryPoint[]
  actualFirstMs: number | null
  actualLastMs: number | null
  totalPoints: number
  returnedPoints: number
  downsampled: boolean
  truncated: boolean
}

export interface NativeHistoryResponse {
  range: NativeHistoryRange
  sinceMs: number
  endMs: number
  host: NativeHistorySeries
  aggregate: NativeHistorySeries
  agents: NativeAgentHistorySeries[]
}

export interface NativeDisplayOption {
  id: string
  label: string
  primary: boolean
}

export interface SessionEventPage {
  sessionId: string
  events: SessionEvent[]
  nextBeforeSequence?: number
  hasMore: boolean
}

export type StreamFrameHandler = (frame: StreamFrame) => void
export type StreamErrorHandler = (error: Error) => void

export interface NativeClient {
  readonly mode: NativeClientMode

  bootstrap(): Promise<BootstrapResult>
  subscribe(onFrame: StreamFrameHandler, onError: StreamErrorHandler): Promise<StreamSubscription>
  openDashboard(): Promise<void>
  openSession(sessionId: string): Promise<void>
  setOverlayMode(mode: OverlayMode): Promise<void>
  acknowledgeLocalAttention(sessionId: string): Promise<void>
  updateSettings(settings: PublicSettings): Promise<PublicSettings>
  purgeHistory(): Promise<void>
  getHistory(range: NativeHistoryRange): Promise<NativeHistoryResponse>
  getSessionEvents(
    sessionId: string,
    beforeSequence?: number,
    limit?: number,
  ): Promise<SessionEventPage>
  listDisplays(): Promise<NativeDisplayOption[]>
  getIntegrationHealth(): Promise<ConnectorHealthReport>
  detectConnectors(): Promise<DetectedConnector[]>
  previewConnector(
    source: AdapterCapabilities['source'],
    scope?: ConnectorScope,
  ): Promise<ConnectorPlanPreview>
  applyConnectorChange(planId: string): Promise<ConnectorApplyResult>
  removeConnector(
    source: AdapterCapabilities['source'],
    scope?: ConnectorScope,
  ): Promise<ConnectorApplyResult>
  repairConnector(
    source: AdapterCapabilities['source'],
    scope?: ConnectorScope,
  ): Promise<ConnectorPlanPreview>
  rollbackConnector(backupId: string): Promise<ConnectorPlanPreview>
  listConnectorBackups(): Promise<BackupJournalEntry[]>
  getPendingDecisions(): Promise<DecisionRequest[]>
  respondDecision(requestId: string, response: DecisionResponse): Promise<DecisionResponseRecord>
}

export interface CreateNativeClientOptions {
  forcePreview?: boolean
}

