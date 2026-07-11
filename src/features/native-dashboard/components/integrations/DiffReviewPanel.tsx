import type { ConnectorPlanPreview } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { agentLabel } from '../../utils/formatters'

type DiffReviewPanelProps = {
  plan: ConnectorPlanPreview
  selectedFilePaths: string[]
  onToggleFile: (displayPath: string, selected: boolean) => void
  onConfirm: () => void
  onCancel: () => void
  confirmLabel?: string | undefined
}

export function DiffReviewPanel({
  plan,
  selectedFilePaths,
  onToggleFile,
  onConfirm,
  onCancel,
  confirmLabel = 'Apply selected files',
}: DiffReviewPanelProps) {
  const selectedSet = new Set(selectedFilePaths)

  return (
    <section className={styles.confirmDialog} aria-label="Integration diff review">
      <h3 className={styles.cardTitle}>Review changes — {agentLabel(plan.source)}</h3>
      <p className={styles.muted}>{plan.summary}</p>
      {plan.backupDisplayHint ? (
        <p className={styles.muted}>Backup hint: {plan.backupDisplayHint}</p>
      ) : null}
      {plan.externalTrustActions.map((action) => (
        <p key={action.kind} className={styles.caveat} role="note">
          {action.instructions}
        </p>
      ))}
      <div className={styles.diffPreview}>
        {plan.files.map((file) => {
          const checked = selectedSet.has(file.displayPath)
          return (
            <article key={file.displayPath} className={styles.card}>
              <label className={styles.checkboxRow}>
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={(event) => onToggleFile(file.displayPath, event.target.checked)}
                />
                <span className={styles.mono}>{file.displayPath}</span>
                {file.isNewFile ? <span className={styles.badgeInfo}>New file</span> : null}
              </label>
              {file.foreignEntriesPreserved.length > 0 ? (
                <p className={styles.muted}>Preserves: {file.foreignEntriesPreserved.join(', ')}</p>
              ) : null}
              <pre className={styles.diffBlock}>{file.diffText}</pre>
            </article>
          )
        })}
      </div>
      <div className={styles.actions}>
        <button type="button" className={styles.button} onClick={onCancel}>
          Cancel
        </button>
        <button
          type="button"
          className={styles.buttonPrimary}
          onClick={onConfirm}
          disabled={selectedFilePaths.length === 0}
        >
          {confirmLabel}
        </button>
      </div>
    </section>
  )
}
