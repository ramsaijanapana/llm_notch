import type { ReactNode } from 'react'
import { useDashboardShortcuts } from '../hooks/useDashboardShortcuts'
import styles from '../styles/dashboard.module.css'
import type { DashboardShellProps, DashboardTab } from '../types/contracts'
import { DashboardTabs } from './DashboardTabs'
import { AgentStatusRail } from './shared/AgentStatusRail'
import { ErrorState } from './shared/ErrorState'
import { LoadingState } from './shared/LoadingState'

const TAB_LABELS: Record<DashboardTab, string> = {
  sessions: 'Sessions',
  metrics: 'Metrics',
  integrations: 'Integrations',
  remote: 'Remote',
  settings: 'Settings',
}

export function DashboardShell({
  loadState,
  errorMessage,
  activeTab,
  onTabChange,
  shortcutsEnabled = true,
  reducedMotion = false,
  agentStatuses = [],
  sessionsPanel,
  metricsPanel,
  integrationsPanel,
  remotePanel,
  settingsPanel,
}: DashboardShellProps) {
  useDashboardShortcuts(activeTab, onTabChange, loadState === 'ready' && shortcutsEnabled)

  const panelByTab: Record<DashboardTab, ReactNode> = {
    sessions: sessionsPanel,
    metrics: metricsPanel,
    integrations: integrationsPanel,
    remote: remotePanel,
    settings: settingsPanel,
  }

  const shellClassName = reducedMotion ? `${styles.shell} ${styles.reduceMotion}` : styles.shell
  const panelClassName = reducedMotion
    ? `${styles.panel} ${styles.panelStatic}`
    : `${styles.panel} ${styles.panelEnter}`

  return (
    <div className={shellClassName} data-testid="dashboard-shell">
      <div className={styles.shellDepth} aria-hidden="true" />
      <aside className={styles.rail}>
        <header className={styles.railBrand}>
          <span className={styles.brandMark} aria-hidden="true" />
          <div className={styles.brandCopy}>
            <span className={styles.title}>LLM Notch</span>
            <span className={styles.brandSubtitle}>Control center</span>
          </div>
        </header>

        <DashboardTabs activeTab={activeTab} onTabChange={onTabChange} />

        <p className={styles.railHint}>
          <span className={styles.railHintLabel}>Shortcuts</span>
          <span className={styles.mono}>⌘/Ctrl+1–5</span>
        </p>
      </aside>

      <div className={styles.main}>
        {agentStatuses.length > 0 ? <AgentStatusRail agents={agentStatuses} /> : null}

        <header className={styles.mainHeader}>
          <h2 className={styles.mainTitle}>{TAB_LABELS[activeTab]}</h2>
        </header>

        <div
          role="tabpanel"
          id={`dashboard-panel-${activeTab}`}
          aria-labelledby={`dashboard-tab-${activeTab}`}
          className={panelClassName}
          key={activeTab}
        >
          {loadState === 'loading' ? <LoadingState /> : null}
          {loadState === 'error' ? (
            <ErrorState message={errorMessage ?? 'Unable to load dashboard.'} />
          ) : null}
          {loadState === 'ready' || loadState === 'empty' ? panelByTab[activeTab] : null}
        </div>
      </div>
    </div>
  )
}
