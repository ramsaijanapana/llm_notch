import { useCallback, useEffect, useRef } from 'react'
import type { DashboardTab } from '../types/contracts'
import { DASHBOARD_TABS } from '../types/contracts'
import { isModifierPressed } from '../utils/formatters'

export function useDashboardShortcuts(
  activeTab: DashboardTab,
  onTabChange: (tab: DashboardTab) => void,
  enabled = true,
): void {
  useEffect(() => {
    if (!enabled) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isModifierPressed(event)) {
        return
      }

      const index = Number.parseInt(event.key, 10)
      if (Number.isNaN(index) || index < 1 || index > DASHBOARD_TABS.length) {
        return
      }

      const nextTab = DASHBOARD_TABS[index - 1]
      if (!nextTab || nextTab === activeTab) {
        return
      }

      event.preventDefault()
      onTabChange(nextTab)
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [activeTab, enabled, onTabChange])
}

type UseRovingTablistOptions<T extends string> = {
  items: readonly T[]
  selectedId: T
  onSelect: (id: T) => void
}

export function useRovingTablist<T extends string>({
  items,
  selectedId,
  onSelect,
}: UseRovingTablistOptions<T>) {
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([])

  const focusTab = useCallback((index: number) => {
    tabRefs.current[index]?.focus()
  }, [])

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
      const lastIndex = items.length - 1
      let nextIndex: number | null = null

      switch (event.key) {
        case 'ArrowLeft':
          nextIndex = index === 0 ? lastIndex : index - 1
          break
        case 'ArrowRight':
          nextIndex = index === lastIndex ? 0 : index + 1
          break
        case 'Home':
          nextIndex = 0
          break
        case 'End':
          nextIndex = lastIndex
          break
        default:
          return
      }

      event.preventDefault()
      const nextItem = items[nextIndex]
      if (nextItem) {
        onSelect(nextItem)
        focusTab(nextIndex)
      }
    },
    [focusTab, items, onSelect],
  )

  const setTabRef = useCallback((index: number, element: HTMLButtonElement | null) => {
    tabRefs.current[index] = element
  }, [])

  return {
    handleKeyDown,
    setTabRef,
    selectedId,
  }
}
