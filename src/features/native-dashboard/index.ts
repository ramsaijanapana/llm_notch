export { DashboardShell } from './components/DashboardShell'
export { DashboardTabs } from './components/DashboardTabs'
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
  AutostartChangeHandler,
  DashboardLoadState,
  DashboardShellProps,
  DashboardTab,
  DisplayChangeHandler,
  DisplayOption,
  HistoryRangeChangeHandler,
  IntegrationActionHandler,
  IntegrationCardState,
  IntegrationDiffPreview,
  IntegrationHealth,
  IntegrationsPanelProps,
  MetricHistoryPoint,
  MetricSeriesCoverage,
  MetricsHistoryBundle,
  MetricsHistoryRange,
  MetricsPanelProps,
  OnboardingFlowProps,
  OnboardingIntegrationChangeHandler,
  OnboardingIntegrationChoice,
  OnboardingStep,
  OpenContextHandler,
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
  activeSessions,
  findAdapterForSession,
  isNotifyOnlyAdapter,
  recentSessions,
  sessionsNeedingAttention,
} from './utils/sessionHelpers'
