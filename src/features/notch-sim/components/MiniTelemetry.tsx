import { Activity, Bell } from 'lucide-react'
import { useSimulation } from '../model/SimulationProvider'
import { activateDemoFromSimulation } from '../model/useDemoAnchor'
import styles from './notchDemo.module.css'
import {
  countActiveSessions,
  countAttentionSessions,
  findSelectedSession,
  getPhaseMeta,
} from './phaseDisplay'

export function MiniTelemetry() {
  const { state, dispatch, prefersReducedMotion } = useSimulation()
  const selected = findSelectedSession(state.sessions, state.selectedId)
  const activeCount = countActiveSessions(state.sessions)
  const attentionCount = countAttentionSessions(state.sessions)

  const phaseMeta = selected ? getPhaseMeta(selected.phase) : null
  const PhaseIcon = phaseMeta?.Icon

  const handleActivate = () => {
    activateDemoFromSimulation(dispatch, prefersReducedMotion)
    document.getElementById('demo')?.scrollIntoView({
      behavior: prefersReducedMotion ? 'auto' : 'smooth',
      block: 'start',
    })
  }

  return (
    <button
      type="button"
      className={styles.miniTelemetry}
      onClick={handleActivate}
      aria-label="Open interactive demo. Shows simulated agent telemetry."
    >
      <div className={styles.miniTelemetryHeader}>
        <span className={styles.miniTelemetryTitle}>Simulated agents</span>
        <div className={styles.miniTelemetryCounts}>
          <span>
            <Activity size={14} aria-hidden="true" />
            {activeCount} active
          </span>
          <span>
            <Bell size={14} aria-hidden="true" />
            {attentionCount} attention
          </span>
        </div>
      </div>

      {selected ? (
        <>
          <p className={styles.miniTelemetryTask}>{selected.task}</p>
          <span
            className={`${styles.miniTelemetryStatus} ${styles[`tone${capitalize(phaseMeta?.tone ?? 'neutral')}`]}`}
          >
            {PhaseIcon ? (
              <PhaseIcon
                size={14}
                aria-hidden="true"
                className={selected.phase === 'running' ? styles.spin : undefined}
              />
            ) : null}
            <span>
              {selected.role} · {phaseMeta?.label}
            </span>
          </span>
        </>
      ) : (
        <p className={styles.miniTelemetryTask}>No session selected</p>
      )}
    </button>
  )
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1)
}
