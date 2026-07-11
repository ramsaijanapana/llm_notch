import type { AdapterCapabilities, AgentSession } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type { OpenContextHandler } from '../../types/contracts'
import { agentLabel, attentionLabel } from '../../utils/formatters'
import { findAdapterForSession, isNotifyOnlyAdapter } from '../../utils/sessionHelpers'
import { MetricStrip } from './MetricStrip'

type SessionDetailProps = {
  session?: AgentSession | undefined
  adapters: AdapterCapabilities[]
  onOpenContext?: OpenContextHandler | undefined
}

export function SessionDetail({ session, adapters, onOpenContext }: SessionDetailProps) {
  if (!session) {
    return (
      <section className={styles.card} aria-label="Session detail">
        <h3 className={styles.sectionTitle}>Session detail</h3>
        <p className={styles.muted}>Select a session to inspect events and metrics.</p>
      </section>
    )
  }

  const adapter = findAdapterForSession(adapters, session)
  const notifyOnly = isNotifyOnlyAdapter(adapter)

  return (
    <section className={styles.card} aria-label="Session detail">
      <h3 className={styles.sectionTitle}>Session detail</h3>
      <div className={styles.capabilityGrid}>
        <div>
          <span className={styles.metricLabel}>Agent</span>
          <p className={styles.listPrimary}>{agentLabel(session.source)}</p>
        </div>
        <div>
          <span className={styles.metricLabel}>Status</span>
          <p className={styles.listPrimary}>{session.status}</p>
        </div>
        <div>
          <span className={styles.metricLabel}>Attention</span>
          <p className={styles.listPrimary}>{attentionLabel(session.attention)}</p>
        </div>
        <div>
          <span className={styles.metricLabel}>Workspace</span>
          <p className={`${styles.listPrimary} ${styles.mono}`}>{session.workspaceLabel ?? '—'}</p>
        </div>
      </div>

      <p className={styles.muted}>{session.label}</p>

      <MetricStrip metric={session.latestMetric} />

      {session.attention !== 'none' ? (
        <div className={styles.actions}>
          {notifyOnly ? (
            <p className={styles.muted} role="status">
              Resolve in {agentLabel(session.source)} — this adapter is notify-only.
            </p>
          ) : null}
          {adapter?.contextOpen && onOpenContext ? (
            <button
              type="button"
              className={styles.buttonPrimary}
              onClick={() => onOpenContext(session.id)}
            >
              Open dashboard context
            </button>
          ) : null}
        </div>
      ) : null}
    </section>
  )
}
