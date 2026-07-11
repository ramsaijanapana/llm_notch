import { useRovingTablist } from '../hooks/useDashboardShortcuts'
import styles from '../styles/dashboard.module.css'
import type { DashboardTab } from '../types/contracts'
import { DASHBOARD_TABS } from '../types/contracts'

const TAB_LABELS: Record<DashboardTab, string> = {
  sessions: 'Sessions',
  metrics: 'Metrics',
  integrations: 'Integrations',
  settings: 'Settings',
}

type DashboardTabsProps = {
  activeTab: DashboardTab
  onTabChange: (tab: DashboardTab) => void
}

export function DashboardTabs({ activeTab, onTabChange }: DashboardTabsProps) {
  const { handleKeyDown, setTabRef } = useRovingTablist({
    items: DASHBOARD_TABS,
    selectedId: activeTab,
    onSelect: onTabChange,
  })

  return (
    <div role="tablist" aria-label="Dashboard sections" className={styles.tabList}>
      {DASHBOARD_TABS.map((tab, index) => {
        const selected = tab === activeTab
        return (
          <button
            key={tab}
            ref={(element) => setTabRef(index, element)}
            type="button"
            role="tab"
            id={`dashboard-tab-${tab}`}
            aria-selected={selected}
            aria-controls={`dashboard-panel-${tab}`}
            tabIndex={selected ? 0 : -1}
            className={styles.tab}
            onClick={() => onTabChange(tab)}
            onKeyDown={(event) => handleKeyDown(event, index)}
          >
            {TAB_LABELS[tab]}
            <span className="sr-only">, shortcut Control or Command {index + 1}</span>
          </button>
        )
      })}
    </div>
  )
}
