import type { BackupJournalEntry } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { agentLabel, formatRelativeTime } from '../../utils/formatters'

type BackupListPanelProps = {
  backups: BackupJournalEntry[]
  onRestore: (backupId: string) => void
  nowMs?: number | undefined
}

export function BackupListPanel({ backups, onRestore, nowMs = Date.now() }: BackupListPanelProps) {
  if (backups.length === 0) {
    return (
      <section className={styles.card} aria-label="Configuration backups">
        <h3 className={styles.sectionTitle}>Backups</h3>
        <p className={styles.muted}>No backups yet. Backups are created before each apply.</p>
      </section>
    )
  }

  return (
    <section className={styles.card} aria-label="Configuration backups">
      <h3 className={styles.sectionTitle}>Backups</h3>
      <ul className={styles.list}>
        {backups.map((backup) => (
          <li key={backup.id} className={styles.actions}>
            <div>
              <p className={styles.listPrimary}>
                {agentLabel(backup.source)} — {backup.displayPath}
              </p>
              <p className={styles.listSecondary}>
                {backup.backupDisplayPath} · {formatRelativeTime(backup.recordedAtMs, nowMs)}
              </p>
            </div>
            <button
              type="button"
              className={styles.button}
              onClick={() => onRestore(backup.id)}
              aria-label={`Restore backup for ${backup.displayPath}`}
            >
              Restore
            </button>
          </li>
        ))}
      </ul>
      <p className={styles.caveat}>
        Restore uses exact rollback when the file hash matches; otherwise you get an additive
        recovery preview.
      </p>
    </section>
  )
}
