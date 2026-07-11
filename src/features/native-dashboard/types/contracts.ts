import type { ReactNode } from 'react'
import type {
  AdapterCapabilities,
  AgentAggregate,
  AgentSession,
  AgentSource,
  HostMetricSample,
  MetricSample,
  PublicSettings,
  SessionEvent,
} from '../../../native/contracts'

export const DASHBOARD_TABS = ['sessions', 'metrics', 'integrations', 'settings'] as const
export type DashboardTab = (typeof DASHBOARD_TABS)[number]

export type DashboardLoadState = 'loading' | 'ready' | 'error' | 'empty'

export type MetricsHistoryRange = '15m' | '1h' | '24h'

export type OnboardingStep = 0 | 1 | 2

export type IntegrationHealth = 'healthy' | 'degraded' | 'offline' | 'unknown'

export type OnboardingIntegrationChoice = AgentSource | 'none'

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
  health: IntegrationHealth
  lastEventAtMs?: number | undefined
  configured: boolean
  previewConfig?: string | undefined
}

export interface IntegrationDiffPreview {
  source: AgentSource
  summary: string
  before: string
  after: string
}

export type TabChangeHandler = (tab: DashboardTab) => void
export type SessionSelectHandler = (sessionId: string) => void
export type OpenContextHandler = (sessionId: string) => void
export type SettingsChangeHandler = (patch: Partial<PublicSettings>) => void
export type IntegrationActionHandler = (source: AgentSource) => void
export type HistoryRangeChangeHandler = (range: MetricsHistoryRange) => void
export type PurgeHistoryHandler = () => void
export type DisplayChangeHandler = (displayId: string | null) => void
export type OnboardingIntegrationChangeHandler = (choice: OnboardingIntegrationChoice) => void
export type AutostartChangeHandler = (enabled: boolean) => void

export interface DashboardShellProps {
  loadState: DashboardLoadState
  errorMessage?: string | undefined
  activeTab: DashboardTab
  onTabChange: TabChangeHandler
  shortcutsEnabled?: boolean | undefined
  reducedMotion?: boolean | undefined
  sessionsPanel: ReactNode
  metricsPanel: ReactNode
  integrationsPanel: ReactNode
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
  selectedIntegration: OnboardingIntegrationChoice
  onIntegrationChange: OnboardingIntegrationChangeHandler
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
  onSelectSession: SessionSelectHandler
  onOpenContext?: OpenContextHandler | undefined
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
}

export interface IntegrationsPanelProps {
  integrations: IntegrationCardState[]
  pendingDiff?: IntegrationDiffPreview | undefined
  writeActionsAvailable?: boolean | undefined
  onPreview: IntegrationActionHandler
  onApply: IntegrationActionHandler
  onRemove: IntegrationActionHandler
  onConfirmDiff: () => void
  onCancelDiff: () => void
  loadState?: DashboardLoadState | undefined
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
  onPurgeHistory: PurgeHistoryHandler
  purgeConfirmOpen?: boolean | undefined
  onPurgeConfirm: () => void
  onPurgeCancel: () => void
  loadState?: DashboardLoadState | undefined
}
