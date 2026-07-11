import styles from '../../styles/dashboard.module.css'
import type { IntegrationsPanelProps } from '../../types/contracts'
import { agentLabel, formatRelativeTime } from '../../utils/formatters'
import { EmptyState } from '../shared/EmptyState'
import { LoadingState } from '../shared/LoadingState'
import { ApplyProgressPanel } from './ApplyProgressPanel'
import { BackupListPanel } from './BackupListPanel'
import { ConnectorHealthBadge } from './ConnectorHealthBadge'
import { DiffReviewPanel } from './DiffReviewPanel'

export function IntegrationsPanel({
  integrations,
  backups,
  pendingPlan,
  applyProgress,
  applyResult,
  writeActionsAvailable = true,
  onConnect,
  onRepair,
  onDisable,
  onConfirmPlan,
  onCancelPlan,
  onTogglePlanFile,
  onRestoreBackup,
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
        description="Connect Cursor, Claude Code, or Codex from this panel after detection."
      />
    )
  }

  return (
    <div className={styles.panelGrid}>
      <div className={styles.cardsRow}>
        {integrations.map((integration) => {
          const { adapter, status, statusDetail, lastEventAtMs, managedEntriesPresent } =
            integration
          const source = adapter.source

          return (
            <article
              key={source}
              className={styles.card}
              aria-label={`${agentLabel(source)} integration`}
            >
              <h3 className={styles.cardTitle}>{agentLabel(source)}</h3>
              <ConnectorHealthBadge
                source={source}
                status={status}
                detail={statusDetail}
              />
              <p className={styles.muted}>
                Last event:{' '}
                {lastEventAtMs ? formatRelativeTime(lastEventAtMs, nowMs) : 'No events yet'}
              </p>
              <p className={styles.muted}>
                llm_notch entries: {managedEntriesPresent ? 'Present' : 'Not installed'}
              </p>
              <section className={styles.capabilityGrid} aria-label="Capability matrix">
                <span>Events: {adapter.events ? 'Yes' : 'No'}</span>
                <span>Attention: {adapter.attention}</span>
                <span>Decisions: {adapter.decisionResponse ? 'In-app' : 'Notify only'}</span>
                <span>Context open: {adapter.contextOpen ? 'Yes' : 'No'}</span>
                <span>Attribution: {adapter.processAttribution}</span>
              </section>
              <div className={styles.actions}>
                {writeActionsAvailable ? (
                  <>
                    <button
                      type="button"
                      className={styles.buttonPrimary}
                      onClick={() => onConnect(source)}
                    >
                      Connect
                    </button>
                    <button type="button" className={styles.button} onClick={() => onRepair(source)}>
                      Repair
                    </button>
                    <button
                      type="button"
                      className={styles.buttonDanger}
                      onClick={() => onDisable(source)}
                    >
                      Disable
                    </button>
                  </>
                ) : (
                  <button type="button" className={styles.button} onClick={() => onConnect(source)}>
                    Preview plan
                  </button>
                )}
              </div>
            </article>
          )
        })}
      </div>

      {pendingPlan ? (
        <DiffReviewPanel
          plan={pendingPlan.plan}
          selectedFilePaths={pendingPlan.selectedFilePaths}
          onToggleFile={(displayPath, selected) => onTogglePlanFile?.(displayPath, selected)}
          onConfirm={onConfirmPlan}
          onCancel={onCancelPlan}
          confirmLabel={writeActionsAvailable ? 'Apply reviewed plan' : 'Close preview'}
        />
      ) : null}

      {applyProgress && applyProgress.length > 0 ? (
        <ApplyProgressPanel progress={applyProgress} result={applyResult} />
      ) : null}

      <BackupListPanel backups={backups} onRestore={onRestoreBackup} nowMs={nowMs} />
    </div>
  )
}
