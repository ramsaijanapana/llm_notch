import styles from '../../styles/dashboard.module.css'
import type { IntegrationsPanelProps } from '../../types/contracts'
import { agentLabel, formatRelativeTime } from '../../utils/formatters'
import { EmptyState } from '../shared/EmptyState'
import { LoadingState } from '../shared/LoadingState'

const HEALTH_LABELS = {
  healthy: 'Healthy',
  degraded: 'Degraded',
  offline: 'Offline',
  unknown: 'Unknown',
} as const

export function IntegrationsPanel({
  integrations,
  pendingDiff,
  writeActionsAvailable = false,
  onPreview,
  onApply,
  onRemove,
  onConfirmDiff,
  onCancelDiff,
  loadState = 'ready',
  nowMs = Date.now(),
}: IntegrationsPanelProps & { nowMs?: number }) {
  if (loadState === 'loading') {
    return <LoadingState label="Loading integrations…" />
  }

  if (loadState === 'empty' || integrations.length === 0) {
    return (
      <EmptyState
        title="No integrations"
        description="Add Cursor, Claude Code, Codex, or Generic adapters from this panel."
      />
    )
  }

  return (
    <div className={styles.panelGrid}>
      <div className={styles.cardsRow}>
        {integrations.map((integration) => {
          const { adapter, health, lastEventAtMs, configured, previewConfig } = integration
          const source = adapter.source

          return (
            <article
              key={source}
              className={styles.card}
              aria-label={`${agentLabel(source)} integration`}
            >
              <h3 className={styles.cardTitle}>{agentLabel(source)}</h3>
              <p className={styles.muted}>
                Health: <span className={styles.badgeInfo}>{HEALTH_LABELS[health]}</span>
              </p>
              <p className={styles.muted}>
                Last event:{' '}
                {lastEventAtMs ? formatRelativeTime(lastEventAtMs, nowMs) : 'No events yet'}
              </p>
              <p className={styles.muted}>Configured: {configured ? 'Yes' : 'No'}</p>
              <section className={styles.capabilityGrid} aria-label="Capability matrix">
                <span>Events: {adapter.events ? 'Yes' : 'No'}</span>
                <span>Attention: {adapter.attention}</span>
                <span>Decisions: {adapter.decisionResponse ? 'In-app' : 'Notify only'}</span>
                <span>Context open: {adapter.contextOpen ? 'Yes' : 'No'}</span>
                <span>Attribution: {adapter.processAttribution}</span>
              </section>
              {previewConfig ? (
                <section aria-label="Config preview">
                  <pre className={styles.diffBlock}>{previewConfig}</pre>
                </section>
              ) : null}
              <div className={styles.actions}>
                <button type="button" className={styles.button} onClick={() => onPreview(source)}>
                  Preview
                </button>
                {writeActionsAvailable ? (
                  <>
                    <button
                      type="button"
                      className={styles.buttonPrimary}
                      onClick={() => onApply(source)}
                    >
                      Apply reviewed plan
                    </button>
                    <button
                      type="button"
                      className={styles.buttonDanger}
                      onClick={() => onRemove(source)}
                    >
                      Remove
                    </button>
                  </>
                ) : null}
              </div>
            </article>
          )
        })}
      </div>

      {pendingDiff ? (
        <section className={styles.confirmDialog} aria-label="Integration diff confirmation">
          <h3 className={styles.cardTitle}>
            {writeActionsAvailable ? 'Confirm configuration change' : 'Read-only template preview'}
          </h3>
          <p className={styles.muted}>{pendingDiff.summary}</p>
          <div className={styles.diffPreview}>
            <p className={styles.metricLabel}>Before</p>
            <pre className={styles.diffBlock}>{pendingDiff.before}</pre>
            <p className={styles.metricLabel}>After</p>
            <pre className={styles.diffBlock}>{pendingDiff.after}</pre>
          </div>
          <div className={styles.actions}>
            <button type="button" className={styles.button} onClick={onCancelDiff}>
              Cancel
            </button>
            <button type="button" className={styles.buttonPrimary} onClick={onConfirmDiff}>
              {writeActionsAvailable ? 'Confirm apply' : 'Close preview'}
            </button>
          </div>
        </section>
      ) : null}

      <p className={styles.caveat}>
        Preview is read-only. Automatic connector file writes are not available in this build; apply
        reviewed templates manually.
      </p>
    </div>
  )
}
