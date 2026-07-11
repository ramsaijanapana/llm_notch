export { DashboardShell } from './components/DashboardShell'
export { DashboardTabs } from './components/DashboardTabs'
export { DecisionSurface } from './components/decisions/DecisionSurface'
export { IntegrationsPanel } from './components/integrations/IntegrationsPanel'
export { MetricsPanel } from './components/metrics/MetricsPanel'
export { OnboardingFlow } from './components/OnboardingFlow'
export { SessionsPanel } from './components/sessions/SessionsPanel'
export { SettingsPanel } from './components/settings/SettingsPanel'
export { EmptyState } from './components/shared/EmptyState'
export { ErrorState } from './components/shared/ErrorState'
export { LoadingState } from './components/shared/LoadingState'
export { QualityBadge } from './components/shared/QualityBadge'
export { SparklineChart } from './components/shared/SparklineChart'
export { useDashboardShortcuts, useRovingTablist } from './hooks/useDashboardShortcuts'
export type {
  AgentMetricHistory,
  ApplyProgressEntry,
  AutostartChangeHandler,
  ConnectFileSelection,
  DashboardLoadState,
  DashboardShellProps,
  DashboardTab,
  DecisionSurfaceProps,
  DisplayChangeHandler,
  DisplayOption,
  HistoryRangeChangeHandler,
  IntegrationActionHandler,
  IntegrationCardState,
  MetricHistoryPoint,
  MetricSeriesCoverage,
  MetricsHistoryBundle,
  MetricsHistoryRange,
  MetricsPanelProps,
  OnboardingFlowProps,
  OnboardingStep,
  OpenContextHandler,
  PendingPlanReview,
  PurgeHistoryHandler,
  SessionSelectHandler,
  SessionsPanelProps,
  SettingsChangeHandler,
  SettingsPanelProps,
  TabChangeHandler,
} from './types/contracts'
export { DASHBOARD_TABS } from './types/contracts'

export {
  agentLabel,
  attentionLabel,
  formatBytes,
  formatBytesPerSec,
  formatDurationMs,
  formatPercent,
  formatRelativeTime,
  historyRangeLabel,
  ioQualityLabel,
  isModifierPressed,
  metricAvailabilityLabel,
} from './utils/formatters'

export {
  connectorStatusGuidance,
  connectorStatusLabel,
  decisionDeliveryLabel,
  DOCUMENTED_CONNECTOR_PATHS,
} from './utils/integrationLabels'

export {
  activeSessions,
  findAdapterForSession,
  isNotifyOnlyAdapter,
  recentSessions,
  sessionsNeedingAttention,
} from './utils/sessionHelpers'
