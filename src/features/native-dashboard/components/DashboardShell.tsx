import type { ReactNode } from 'react'
import { useDashboardShortcuts } from '../hooks/useDashboardShortcuts'
import styles from '../styles/dashboard.module.css'
import type { DashboardShellProps, DashboardTab } from '../types/contracts'
import { DashboardTabs } from './DashboardTabs'
import { ErrorState } from './shared/ErrorState'
import { LoadingState } from './shared/LoadingState'

export function DashboardShell({
  loadState,
  errorMessage,
  activeTab,
  onTabChange,
  shortcutsEnabled = true,
  reducedMotion = false,
  sessionsPanel,
  metricsPanel,
  integrationsPanel,
  settingsPanel,
}: DashboardShellProps) {
  useDashboardShortcuts(activeTab, onTabChange, loadState === 'ready' && shortcutsEnabled)

  const panelByTab: Record<DashboardTab, ReactNode> = {
    sessions: sessionsPanel,
    metrics: metricsPanel,
    integrations: integrationsPanel,
    settings: settingsPanel,
  }

  const shellClassName = reducedMotion ? `${styles.shell} ${styles.reduceMotion}` : styles.shell

  return (
    <div className={shellClassName} data-testid="dashboard-shell">
      <header className={styles.header}>
        <h1 className={styles.title}>LLM Notch</h1>
        <p className={styles.shortcutHint}>Ctrl/Cmd+1–4 switch tabs</p>
      </header>

      <div className={styles.body}>
        <DashboardTabs activeTab={activeTab} onTabChange={onTabChange} />

        <div
          role="tabpanel"
          id={`dashboard-panel-${activeTab}`}
          aria-labelledby={`dashboard-tab-${activeTab}`}
          className={styles.panel}
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
