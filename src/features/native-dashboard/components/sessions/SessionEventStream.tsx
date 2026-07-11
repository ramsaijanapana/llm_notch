import type { SessionEvent } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { formatRelativeTime } from '../../utils/formatters'

type SessionEventStreamProps = {
  events: SessionEvent[]
  nowMs?: number | undefined
}

export function SessionEventStream({ events, nowMs = Date.now() }: SessionEventStreamProps) {
  return (
    <section className={styles.card} aria-label="Session event stream">
      <h3 className={styles.sectionTitle}>Event stream</h3>
      <div className={styles.eventLog} role="log" aria-live="polite" aria-relevant="additions">
        {events.length === 0 ? (
          <p className={styles.muted}>No events for this session yet.</p>
        ) : (
          events.map((event) => (
            <p key={event.id} className={styles.eventEntry}>
              <time
                className={styles.eventTime}
                dateTime={new Date(event.occurredAtMs).toISOString()}
              >
                {formatRelativeTime(event.occurredAtMs, nowMs)}
              </time>
              <span className={styles.badgeInfo}>{event.kind}</span> {event.summary}
              {event.toolName ? <span className={styles.mono}> ({event.toolName})</span> : null}
            </p>
          ))
        )}
      </div>
    </section>
  )
}
