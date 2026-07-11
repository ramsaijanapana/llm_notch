import { useCallback, useRef } from 'react'
import { useSimulation } from '../model/SimulationProvider'
import type { AgentSession } from '../model/simulation.types'
import styles from './notchDemo.module.css'
import { getPhaseMeta } from './phaseDisplay'

type SessionTabsProps = {
  sessions: AgentSession[]
  selectedId: string
}

export function SessionTabs({ sessions, selectedId }: SessionTabsProps) {
  const { dispatch } = useSimulation()
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([])

  const selectSession = useCallback(
    (sessionId: string) => {
      dispatch({ type: 'SELECT_SESSION', sessionId })
    },
    [dispatch],
  )

  const focusTab = (index: number) => {
    const tab = tabRefs.current[index]
    tab?.focus()
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
    const lastIndex = sessions.length - 1
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
    const nextSession = sessions[nextIndex]
    if (nextSession) {
      selectSession(nextSession.id)
      focusTab(nextIndex)
    }
  }

  return (
    <div role="tablist" aria-label="Agent sessions" className={styles.tabList}>
      {sessions.map((session, index) => {
        const isSelected = session.id === selectedId
        const { label, Icon, tone } = getPhaseMeta(session.phase)

        return (
          <button
            key={session.id}
            ref={(element) => {
              tabRefs.current[index] = element
            }}
            type="button"
            role="tab"
            id={`session-tab-${session.id}`}
            aria-selected={isSelected}
            aria-controls={`session-panel-${session.id}`}
            tabIndex={isSelected ? 0 : -1}
            className={styles.tab}
            onClick={() => selectSession(session.id)}
            onKeyDown={(event) => handleKeyDown(event, index)}
          >
            <span className={`${styles.tabIcon} ${styles[`tone${capitalize(tone)}`]}`}>
              <Icon
                size={16}
                aria-hidden="true"
                className={session.phase === 'running' ? styles.spin : undefined}
              />
            </span>
            <span className={styles.tabLabel}>
              {session.role}
              <span className="sr-only">, {label}</span>
            </span>
          </button>
        )
      })}
    </div>
  )
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1)
}
