import type { AgentSession } from '../model/simulation.types'
import styles from './notchDemo.module.css'
import { formatCost, formatElapsed, getPhaseMeta } from './phaseDisplay'

type SessionDetailProps = {
  session: AgentSession
}

export function SessionDetail({ session }: SessionDetailProps) {
  const { label, Icon, tone } = getPhaseMeta(session.phase)

  return (
    <section className={styles.panel}>
      <h3 className={styles.panelTitle}>Session detail</h3>

      <div className={styles.metaGrid}>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Agent role</span>
          <span className={styles.metaValue}>{session.role}</span>
        </div>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Status</span>
          <span className={`${styles.metaValue} ${styles[`tone${capitalize(tone)}`]}`}>
            <Icon
              size={14}
              aria-hidden="true"
              className={session.phase === 'running' ? styles.spin : undefined}
            />{' '}
            {label}
          </span>
        </div>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Task</span>
          <span className={styles.metaValue}>{session.task}</span>
        </div>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Workspace</span>
          <span className={`${styles.metaValue} ${styles.metaValueMono}`}>{session.workspace}</span>
        </div>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Tokens</span>
          <span className={`${styles.metaValue} ${styles.metaValueMono}`}>
            {session.tokenCount.toLocaleString()}
          </span>
        </div>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Simulated cost</span>
          <span className={`${styles.metaValue} ${styles.metaValueMono}`}>
            {formatCost(session.costCents)}
          </span>
        </div>
        <div className={styles.metaItem}>
          <span className={styles.metaLabel}>Elapsed</span>
          <span className={`${styles.metaValue} ${styles.metaValueMono}`}>
            {formatElapsed(session.elapsedSeconds)}
          </span>
        </div>
      </div>

      <div className={styles.progressBlock}>
        <div className={styles.progressHeader}>
          <span>Progress</span>
          <span>{session.progress}%</span>
        </div>
        <div
          className={styles.progressTrack}
          role="progressbar"
          aria-valuenow={session.progress}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label="Task progress"
        >
          <div className={styles.progressFill} style={{ width: `${session.progress}%` }} />
        </div>
      </div>

      <div>
        <h4 className={styles.panelTitle}>Event log</h4>
        <div className={styles.eventLog} role="log" aria-label="Session event log">
          {session.events.length === 0 ? (
            <p className={styles.eventEntry}>No events yet.</p>
          ) : (
            session.events.map((event) => (
              <p key={event.id} className={styles.eventEntry}>
                <time dateTime={`PT${event.timestamp}S`}>{formatElapsed(event.timestamp)}</time>
                <span>[{event.actor}] </span>
                {event.message}
              </p>
            ))
          )}
        </div>
      </div>
    </section>
  )
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1)
}
