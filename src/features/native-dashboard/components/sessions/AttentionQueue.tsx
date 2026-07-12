import type { AgentSession } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { agentLabel, attentionLabel } from '../../utils/formatters'

type AttentionQueueProps = {
  sessions: AgentSession[]
  selectedSessionId?: string | undefined
  onSelectSession: (sessionId: string) => void
  onAcknowledge?: ((sessionId: string) => void) | undefined
}

export function AttentionQueue({
  sessions,
  selectedSessionId,
  onSelectSession,
  onAcknowledge,
}: AttentionQueueProps) {
  if (sessions.length === 0) {
    return (
      <section className={styles.card} aria-label="Attention queue">
        <h3 className={styles.sectionTitle}>Attention queue</h3>
        <p className={styles.muted}>No sessions need attention.</p>
      </section>
    )
  }

  return (
    <section className={styles.card} aria-label="Attention queue">
      <h3 className={styles.sectionTitle}>Attention queue</h3>
      <ul className={styles.list}>
        {sessions.map((session) => (
          <li key={session.id} className={styles.attentionRow}>
            <button
              type="button"
              className={styles.listButton}
              aria-current={session.id === selectedSessionId}
              onClick={() => onSelectSession(session.id)}
            >
              <span className={styles.listPrimary}>{session.label}</span>
              <span className={styles.listSecondary}>
                {agentLabel(session.source)} · {attentionLabel(session.attention)}
              </span>
            </button>
            {onAcknowledge ? (
              <button
                type="button"
                className={styles.button}
                aria-label={`Acknowledge ${session.label}`}
                onClick={() => onAcknowledge(session.id)}
              >
                Acknowledge
              </button>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  )
}
