import type { ReactNode } from 'react'
import type {
  AdapterCapabilities,
  AgentAggregate,
  AgentCatalogEntry,
  AgentSession,
  AgentSource,
  BackupJournalEntry,
  ConnectorApplyResult,
  ConnectorFileApplyResult,
  ConnectorFilePreview,
  ConnectorPlanPreview,
  ConnectorScope,
  ConnectorUserStatus,
  DecisionDeliveryState,
  DecisionKind,
  DecisionRequest,
  DecisionResponseRecord,
  DetectedConnector,
  HostMetricSample,
  MetricSample,
  PublicSettings,
  QuotaSnapshotView,
  RemoteBackendStatus,
  RemoteDeploymentPlanView,
  RemoteDeploymentResultView,
  RemoteHostConfigInput,
  RemoteHostView,
  SessionEvent,
} from '../../../native/contracts'

export const DASHBOARD_TABS = ['sessions', 'metrics', 'integrations', 'remote', 'settings'] as const
export type DashboardTab = (typeof DASHBOARD_TABS)[number]

export type DashboardLoadState = 'loading' | 'ready' | 'error' | 'empty'

export type MetricsHistoryRange = '15m' | '1h' | '24h'

export type OnboardingStep = 0 | 1 | 2 | 3 | 4

export type OnboardingIntegrationChoice = AgentSource | 'none'

export type ApplyProgressPhase =
  | 'applying'
  | 'backingUp'
  | 'writing'
  | 'verifying'
  | 'done'
  | 'failed'

export interface ApplyProgressEntry {
  displayPath: string
  phase: ApplyProgressPhase
  message?: string | undefined
}

export interface DisplayOption {
  id: string
  label: string
  primary?: boolean | undefined
}

export interface MetricHistoryPoint {
  atMs: number
  value: number
}

export interface AgentMetricHistory {
  sessionId: string
  source: AgentSource
  label: string
  cpu: MetricHistoryPoint[]
  rss: MetricHistoryPoint[]
  coverage: MetricSeriesCoverage
}

export interface MetricSeriesCoverage {
  requestedStartMs: number
  requestedEndMs: number
  actualFirstMs?: number | undefined
  actualLastMs?: number | undefined
  totalPoints: number
  returnedPoints: number
  downsampled: boolean
  truncated: boolean
}

export interface MetricsHistoryBundle {
  requestedStartMs: number
  requestedEndMs: number
  hostCpu: MetricHistoryPoint[]
  aggregateCpu: MetricHistoryPoint[]
  aggregateRss: MetricHistoryPoint[]
  hostCoverage: MetricSeriesCoverage
  aggregateCoverage: MetricSeriesCoverage
  perAgent: AgentMetricHistory[]
}

export interface IntegrationCardState {
  adapter: AdapterCapabilities
  status: ConnectorUserStatus
  statusDetail?: string | undefined
  lastEventAtMs?: number | undefined
  managedEntriesPresent: boolean
}

export interface ConnectFileSelection {
  source: AgentSource
  displayPath: string
  selected: boolean
}

export interface PendingPlanReview {
  plan: ConnectorPlanPreview
  selectedFilePaths: string[]
}

export type TabChangeHandler = (tab: DashboardTab) => void
export type SessionSelectHandler = (sessionId: string) => void
export type OpenContextHandler = (sessionId: string) => void
export type SettingsChangeHandler = (patch: Partial<PublicSettings>) => void
export type IntegrationActionHandler = (source: AgentSource) => void
export type HistoryRangeChangeHandler = (range: MetricsHistoryRange) => void
export type PurgeHistoryHandler = () => void
export type PurgeScopeChangeHandler = (
  patch: Partial<import('../../../native/contracts').PurgeScope>,
) => void
export type DisplayChangeHandler = (displayId: string | null) => void
export type OnboardingIntegrationChangeHandler = (choice: OnboardingIntegrationChoice) => void
export type AutostartChangeHandler = (enabled: boolean) => void

export interface AgentStatusEntry {
  source: AgentSource
  status: ConnectorUserStatus
  activeSessions?: number | undefined
  attentionSessions?: number | undefined
}

export interface DashboardShellProps {
  loadState: DashboardLoadState
  errorMessage?: string | undefined
  activeTab: DashboardTab
  onTabChange: TabChangeHandler
  shortcutsEnabled?: boolean | undefined
  reducedMotion?: boolean | undefined
  agentStatuses?: AgentStatusEntry[] | undefined
  sessionsPanel: ReactNode
  metricsPanel: ReactNode
  integrationsPanel: ReactNode
  remotePanel: ReactNode
  settingsPanel: ReactNode
}

export interface OnboardingFlowProps {
  open: boolean
  step: OnboardingStep
  displays: DisplayOption[]
  selectedDisplayId?: string | null | undefined
  displayLoadState?: 'loading' | 'ready' | 'error' | undefined
  displayError?: string | undefined
  fullscreenPreferenceSupported?: boolean | undefined
  onDisplayChange: DisplayChangeHandler
  integrationOptions: AgentSource[]
  detectedConnectors: DetectedConnector[]
  detectLoadState?: 'idle' | 'loading' | 'ready' | 'error' | undefined
  detectError?: string | undefined
  onGetStarted: () => void
  connectSelections: ConnectFileSelection[]
  onConnectSelectionChange: (selections: ConnectFileSelection[]) => void
  connectScope: ConnectorScope
  onConnectScopeChange: (scope: ConnectorScope) => void
  pendingPlan?: PendingPlanReview | undefined
  pendingPlanCount?: number | undefined
  applyProgress?: ApplyProgressEntry[] | undefined
  applyResult?: ConnectorApplyResult | undefined
  onPreviewConnect: () => void
  onConfirmApply: () => void
  onSkipConnect: () => void
  onTogglePlanFile?: ((displayPath: string, selected: boolean) => void) | undefined
  shortcutLabel: string
  autostartEnabled: boolean
  onAutostartChange: AutostartChangeHandler
  onNext: () => void
  onBack: () => void
  onSkip: () => void
  onFinish: () => void
  reducedMotion?: boolean | undefined
}

export interface SessionsPanelProps {
  sessions: AgentSession[]
  selectedSessionId?: string | undefined
  events: SessionEvent[]
  adapters: AdapterCapabilities[]
  pendingDecision?: DecisionRequest | undefined
  decisionRecord?: DecisionResponseRecord | undefined
  onSelectSession: SessionSelectHandler
  onOpenContext?: OpenContextHandler | undefined
  onDecisionAllow?: (() => void) | undefined
  onDecisionDeny?: (() => void) | undefined
  onDecisionAnswer?: ((text: string) => void) | undefined
  loadState?: DashboardLoadState | undefined
  emptyMessage?: string | undefined
}

export interface MetricsPanelProps {
  host?: HostMetricSample | undefined
  aggregate?: AgentAggregate | undefined
  agents: Record<string, MetricSample>
  history: MetricsHistoryBundle
  historyRange: MetricsHistoryRange
  onHistoryRangeChange: HistoryRangeChangeHandler
  loadState?: DashboardLoadState | undefined
  warmingUp?: boolean | undefined
  historyLoadState?: DashboardLoadState | undefined
  historyError?: string | undefined
  disabledHistoryRanges?: MetricsHistoryRange[] | undefined
  quotas?: QuotaSnapshotView[] | undefined
  onRefreshQuotas?: (() => void) | undefined
  quotaRefreshState?: 'idle' | 'loading' | undefined
}

export interface IntegrationsPanelProps {
  integrations: IntegrationCardState[]
  catalog?: AgentCatalogEntry[] | undefined
  backups: BackupJournalEntry[]
  pendingPlan?: PendingPlanReview | undefined
  applyProgress?: ApplyProgressEntry[] | undefined
  applyResult?: ConnectorApplyResult | undefined
  writeActionsAvailable?: boolean | undefined
  onConnect: IntegrationActionHandler
  onRepair: IntegrationActionHandler
  onDisable: IntegrationActionHandler
  onConfirmPlan: () => void
  onCancelPlan: () => void
  onTogglePlanFile?: ((displayPath: string, selected: boolean) => void) | undefined
  onRestoreBackup: (backupId: string) => void
  loadState?: DashboardLoadState | undefined
}

export interface DecisionSurfaceProps {
  request: DecisionRequest
  adapter: AdapterCapabilities | undefined
  deliveryRecord?: DecisionResponseRecord | undefined
  onAllow?: (() => void) | undefined
  onDeny?: (() => void) | undefined
  onAnswer?: ((text: string) => void) | undefined
}

export interface RemotePanelProps {
  hosts: RemoteHostView[]
  sessions?: AgentSession[] | undefined
  backendStatus: RemoteBackendStatus
  pendingDeployPlan?: RemoteDeploymentPlanView | undefined
  pendingDeployResult?: RemoteDeploymentResultView | undefined
  deployBusy?: boolean | undefined
  loadState?: DashboardLoadState | undefined
  lifecycleActionsAvailable?: boolean | undefined
  hostConfigActionsAvailable?: boolean | undefined
  onPlanDeploy: (hostId: string) => void
  onExecuteDeploy?: ((hostId: string) => void) | undefined
  onStartRelay: (hostId: string) => void
  onStopRelay: (hostId: string) => void
  onDismissPlan: () => void
  onAddHost?: ((config: RemoteHostConfigInput) => void) | undefined
  onRemoveHost?: ((hostId: string) => void) | undefined
}

export interface SettingsPanelProps {
  settings: PublicSettings
  displays: DisplayOption[]
  displayLoadState?: 'loading' | 'ready' | 'error' | undefined
  displayError?: string | undefined
  fullscreenPreferenceSupported?: boolean | undefined
  onDisplayChange: DisplayChangeHandler
  shortcutLabel: string
  onSettingsChange: SettingsChangeHandler
  soundThemes?: import('../../../native/contracts').SoundTheme[] | undefined
  soundImportMessage?: string | undefined
  soundImportError?: string | undefined
  soundImportBusy?: boolean | undefined
  onImportSoundPack?: ((file: File) => void) | undefined
  onPreviewSoundTheme?: ((themeId: string, event: import('../../../native/contracts').SoundEvent) => void) | undefined
  soundPlaybackSupported?: boolean | undefined
  purgeScope?: import('../../../native/contracts').PurgeScope | undefined
  onPurgeScopeChange?: PurgeScopeChangeHandler | undefined
  onPurgeHistory: PurgeHistoryHandler
  purgeConfirmOpen?: boolean | undefined
  onPurgeConfirm: () => void
  onPurgeCancel: () => void
  loadState?: DashboardLoadState | undefined
}

export type {
  ConnectorFileApplyResult,
  ConnectorFilePreview,
  DecisionDeliveryState,
  DecisionKind,
  RemoteHostConfigInput,
}
