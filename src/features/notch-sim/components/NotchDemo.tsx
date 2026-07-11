import { useLayoutEffect, useRef } from 'react'
import { useSimulation } from '../model/SimulationProvider'
import { DecisionPanel } from './DecisionPanel'
import { DemoControls } from './DemoControls'
import styles from './notchDemo.module.css'
import {
  countActiveSessions,
  countAttentionSessions,
  findSelectedSession,
  getPhaseMeta,
} from './phaseDisplay'
import { SessionDetail } from './SessionDetail'
import { SessionTabs } from './SessionTabs'
import { TerminalDrawer } from './TerminalDrawer'

function capsuleStatusLabel(
  expanded: boolean,
  playing: boolean,
  activeCount: number,
  attentionCount: number,
): string {
  const mode = expanded ? 'expanded' : 'collapsed'
  const playback = playing ? 'playing' : 'paused'
  return `Interactive demo console, ${mode}, ${playback}. ${activeCount} active sessions, ${attentionCount} need attention.`
}

export function NotchDemo() {
  const { state, dispatch } = useSimulation()
  const jumpTriggerRef = useRef<HTMLButtonElement>(null)
  const capsuleRef = useRef<HTMLButtonElement>(null)
  const focusCapsuleAfterResetRef = useRef(false)

  useLayoutEffect(() => {
    if (!focusCapsuleAfterResetRef.current || state.expanded) {
      return
    }

    focusCapsuleAfterResetRef.current = false
    capsuleRef.current?.focus()
  }, [state.expanded])
  const selected = findSelectedSession(state.sessions, state.selectedId)
  const activeCount = countActiveSessions(state.sessions)
  const attentionCount = countAttentionSessions(state.sessions)

  return (
    <section id="demo" className={styles.demo} aria-labelledby="demo-heading">
      <div className={styles.demoIntro}>
        <h2 id="demo-heading">Interactive edge console (simulated)</h2>
        <p>
          Browse simulated agent sessions, approvals, and telemetry locally — nothing connects to a
          real runtime. <span className={styles.simulationBadge}>Simulation only</span>
        </p>
      </div>

      <div className={styles.workstation}>
        <button
          ref={capsuleRef}
          type="button"
          className={styles.consoleCapsule}
          onClick={() => dispatch({ type: 'TOGGLE_EXPANDED' })}
          aria-expanded={state.expanded}
          aria-controls="demo-workstation-body"
          aria-label={capsuleStatusLabel(
            state.expanded,
            state.playing,
            activeCount,
            attentionCount,
          )}
        >
          <span className={styles.consoleCapsuleLead}>
            <span className={styles.consoleDot} aria-hidden="true" />
            <span className={styles.consoleLabel}>llm_notch · demo</span>
          </span>

          {state.expanded ? (
            <span className={styles.consoleMeta}>
              <span>{activeCount} active</span>
              <span>{attentionCount} need attention</span>
              <span>{state.playing ? 'playing' : 'paused'}</span>
            </span>
          ) : (
            <ul className={styles.sessionStatusDots} aria-label="Session status">
              {state.sessions.map((session) => {
                const { label, tone } = getPhaseMeta(session.phase)
                return (
                  <li key={session.id} className={styles.sessionStatusItem}>
                    <span
                      className={`${styles.sessionDot} ${styles[`tone${capitalize(tone)}`]}`}
                      aria-hidden="true"
                    />
                    <span className={styles.sessionStatusLabel} aria-hidden="true">
                      {session.role}
                    </span>
                    <span className="sr-only">
                      {session.role}: {label}
                    </span>
                  </li>
                )
              })}
            </ul>
          )}
        </button>

        {state.expanded ? (
          <DemoControls
            onReset={() => {
              focusCapsuleAfterResetRef.current = true
            }}
          />
        ) : null}

        <div id="demo-workstation-body" className={styles.workstationBody} hidden={!state.expanded}>
          {state.expanded ? (
            <SessionTabs sessions={state.sessions} selectedId={state.selectedId} />
          ) : null}

          {state.sessions.map((session) => (
            <div
              key={session.id}
              role="tabpanel"
              id={`session-panel-${session.id}`}
              aria-labelledby={`session-tab-${session.id}`}
              hidden={!state.expanded || session.id !== state.selectedId}
              className={styles.tabPanel}
            >
              {state.expanded && session.id === state.selectedId ? (
                <div className={styles.detailGrid}>
                  <SessionDetail session={session} />
                  <DecisionPanel session={session} jumpTriggerRef={jumpTriggerRef} />
                </div>
              ) : null}
            </div>
          ))}

          {state.expanded && !selected ? <p>Select a session tab to view details.</p> : null}
        </div>

        <TerminalDrawer jumpTriggerRef={jumpTriggerRef} />
      </div>

      <div role="status" aria-live="polite" aria-atomic="true" className="sr-only">
        {state.announcement ?? ''}
      </div>
    </section>
  )
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1)
}
