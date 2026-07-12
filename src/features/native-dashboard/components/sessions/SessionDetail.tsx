import type { AdapterCapabilities, AgentSession } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type { OpenContextHandler } from '../../types/contracts'
import type { DecisionRequest, DecisionResponseRecord } from '../../../../native/contracts'
import { agentLabel, attentionLabel } from '../../utils/formatters'
import { findAdapterForSession, isNotifyOnlyAdapter, decisionMatchesSession } from '../../utils/sessionHelpers'
import { DecisionSurface } from '../decisions/DecisionSurface'
import { MetricStrip } from './MetricStrip'

type SessionDetailProps = {
  session?: AgentSession | undefined
  adapters: AdapterCapabilities[]
  pendingDecision?: DecisionRequest | undefined
  decisionRecord?: DecisionResponseRecord | undefined
  onOpenContext?: OpenContextHandler | undefined
  onDecisionAllow?: (() => void) | undefined
  onDecisionDeny?: (() => void) | undefined
  onDecisionAnswer?: ((text: string) => void) | undefined
}

export function SessionDetail({
  session,
  adapters,
  pendingDecision,
  decisionRecord,
  onOpenContext,
  onDecisionAllow,
  onDecisionDeny,
  onDecisionAnswer,
}: SessionDetailProps) {
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
  const showDecision =
    pendingDecision !== undefined && decisionMatchesSession(pendingDecision, session)

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

      {showDecision ? (
        <DecisionSurface
          request={pendingDecision}
          adapter={adapter}
          deliveryRecord={decisionRecord}
          onAllow={onDecisionAllow}
          onDeny={onDecisionDeny}
          onAnswer={onDecisionAnswer}
        />
      ) : null}

      {session.attention !== 'none' && !showDecision ? (
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
