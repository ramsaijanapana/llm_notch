import { Terminal, X } from 'lucide-react'
import type { RefObject } from 'react'
import { useSimulation } from '../model/SimulationProvider'
import styles from './notchDemo.module.css'
import { findSelectedSession } from './phaseDisplay'

type TerminalDrawerProps = {
  jumpTriggerRef?: RefObject<HTMLButtonElement | null>
}

export function TerminalDrawer({ jumpTriggerRef }: TerminalDrawerProps) {
  const { state, dispatch } = useSimulation()

  if (!state.terminalOpen) {
    return null
  }

  const selected = findSelectedSession(state.sessions, state.selectedId)

  const handleClose = () => {
    dispatch({ type: 'CLOSE_TERMINAL' })
    jumpTriggerRef?.current?.focus()
  }

  return (
    <aside className={styles.terminalDrawer} aria-label="Simulated terminal">
      <div className={styles.terminalHeader}>
        <span className={styles.terminalBadge}>
          <Terminal size={14} aria-hidden="true" />
          Simulation only
        </span>
        <button
          type="button"
          className={styles.btn}
          onClick={handleClose}
          aria-label="Close simulated terminal"
        >
          <X size={16} aria-hidden="true" />
          Close
        </button>
      </div>

      <div className={styles.terminalBody}>
        <p className={styles.terminalLine}>
          <span className={styles.terminalPrompt}>$</span> notch-sim jump --workspace{' '}
          {selected?.workspace ?? 'unknown'}
        </p>
        <p className={styles.terminalLine}>Connecting to simulated workspace context…</p>
        <p className={styles.terminalLine}>
          &gt; Listing files in {selected?.workspace ?? 'workspace'}
        </p>
        <p className={styles.terminalLine}>&gt; Reading task: {selected?.task ?? 'n/a'}</p>
        <p className={styles.terminalLine}>&gt; Typecheck (simulated) … ok</p>
        <p className={styles.terminalLine}>
          [Simulation only — no shell, network, or filesystem access]
        </p>
      </div>
    </aside>
  )
}
