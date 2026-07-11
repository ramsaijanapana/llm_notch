import type { AgentSession } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { agentLabel } from '../../utils/formatters'

type SessionListProps = {
  title: string
  sessions: AgentSession[]
  selectedSessionId?: string | undefined
  onSelectSession: (sessionId: string) => void
  emptyLabel: string
}

export function SessionList({
  title,
  sessions,
  selectedSessionId,
  onSelectSession,
  emptyLabel,
}: SessionListProps) {
  return (
    <section className={styles.card} aria-label={title}>
      <h3 className={styles.sectionTitle}>{title}</h3>
      {sessions.length === 0 ? (
        <p className={styles.muted}>{emptyLabel}</p>
      ) : (
        <ul className={styles.list}>
          {sessions.map((session) => (
            <li key={session.id}>
              <button
                type="button"
                className={styles.listButton}
                aria-current={session.id === selectedSessionId}
                onClick={() => onSelectSession(session.id)}
              >
                <span className={styles.listPrimary}>{session.label}</span>
                <span className={styles.listSecondary}>
                  {agentLabel(session.source)} · {session.status}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
