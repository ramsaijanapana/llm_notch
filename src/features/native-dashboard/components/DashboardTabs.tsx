import { Activity, LayoutList, PlugZap, Server, SlidersHorizontal } from 'lucide-react'
import { useRovingTablist } from '../hooks/useDashboardShortcuts'
import styles from '../styles/dashboard.module.css'
import type { DashboardTab } from '../types/contracts'
import { DASHBOARD_TABS } from '../types/contracts'

const TAB_CONFIG: Record<
  DashboardTab,
  { label: string; icon: typeof LayoutList; shortcut: number }
> = {
  sessions: { label: 'Sessions', icon: LayoutList, shortcut: 1 },
  metrics: { label: 'Metrics', icon: Activity, shortcut: 2 },
  integrations: { label: 'Integrations', icon: PlugZap, shortcut: 3 },
  remote: { label: 'Remote', icon: Server, shortcut: 4 },
  settings: { label: 'Settings', icon: SlidersHorizontal, shortcut: 5 },
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
    orientation: 'vertical',
  })

  return (
    <div role="tablist" aria-label="Dashboard sections" className={styles.railTabList}>
      {DASHBOARD_TABS.map((tab, index) => {
        const selected = tab === activeTab
        const { label, icon: Icon, shortcut } = TAB_CONFIG[tab]
        return (
          <button
            key={tab}
            ref={(element) => setTabRef(index, element)}
            type="button"
            role="tab"
            id={`dashboard-tab-${tab}`}
            aria-selected={selected}
            aria-controls={`dashboard-panel-${tab}`}
            aria-label={label}
            tabIndex={selected ? 0 : -1}
            className={selected ? `${styles.railTab} ${styles.railTabActive}` : styles.railTab}
            onClick={() => onTabChange(tab)}
            onKeyDown={(event) => handleKeyDown(event, index)}
            title={label}
          >
            <Icon size={16} strokeWidth={2} aria-hidden="true" />
            <span className={styles.railTabLabel}>{label}</span>
            <span className="sr-only">, shortcut Control or Command {shortcut}</span>
          </button>
        )
      })}
    </div>
  )
}
