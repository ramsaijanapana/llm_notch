import { useState } from 'react'
import styles from '../../styles/dashboard.module.css'
import type { DecisionSurfaceProps } from '../../types/contracts'
import { agentLabel } from '../../utils/formatters'
import { decisionDeliveryLabel } from '../../utils/integrationLabels'

export function DecisionSurface({
  request,
  adapter,
  deliveryRecord,
  onAllow,
  onDeny,
  onAnswer,
}: DecisionSurfaceProps) {
  const [answerText, setAnswerText] = useState('')
  const controlsEnabled =
    adapter?.decisionResponse === true &&
    request.hasActionablePayload &&
    deliveryRecord === undefined

  return (
    <section className={styles.card} aria-label="Agent decision" data-testid="decision-surface">
      <h3 className={styles.cardTitle}>
        {request.kind === 'question' ? 'Question' : 'Approval'} — {agentLabel(request.source)}
      </h3>
      <p className={styles.muted}>{request.summary}</p>

      {deliveryRecord ? (
        <p className={styles.muted} role="status">
          {decisionDeliveryLabel(deliveryRecord.deliveryState)}
          {deliveryRecord.deliveryDetail ? ` — ${deliveryRecord.deliveryDetail}` : ''}
        </p>
      ) : adapter?.decisionResponse === false ? (
        <p className={styles.caveat} role="status">
          Resolve in {agentLabel(request.source)} — this adapter cannot receive in-app responses.
          Open the agent to continue.
        </p>
      ) : !request.hasActionablePayload ? (
        <p className={styles.caveat} role="status">
          Waiting for agent payload — controls appear when the hook delivers actionable content.
        </p>
      ) : null}

      {controlsEnabled && request.kind !== 'question' ? (
        <div className={styles.actions}>
          <button type="button" className={styles.buttonPrimary} onClick={onAllow}>
            Allow
          </button>
          <button type="button" className={styles.buttonDanger} onClick={onDeny}>
            Deny
          </button>
        </div>
      ) : null}

      {controlsEnabled && request.kind === 'question' ? (
        <div className={styles.field}>
          <label htmlFor="decision-answer">Your answer</label>
          <textarea
            id="decision-answer"
            className={styles.input}
            rows={3}
            value={answerText}
            onChange={(event) => setAnswerText(event.target.value)}
          />
          <div className={styles.actions}>
            <button
              type="button"
              className={styles.buttonPrimary}
              onClick={() => onAnswer?.(answerText)}
              disabled={answerText.trim().length === 0}
            >
              Submit answer
            </button>
          </div>
        </div>
      ) : null}
    </section>
  )
}
