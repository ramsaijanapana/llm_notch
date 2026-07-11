import type { ConnectorApplyResult } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type { ApplyProgressEntry } from '../../types/contracts'
import { applyProgressLabel } from '../../utils/integrationLabels'

type ApplyProgressPanelProps = {
  progress: ApplyProgressEntry[]
  result?: ConnectorApplyResult | undefined
}

function phaseClass(phase: ApplyProgressEntry['phase']): string {
  switch (phase) {
    case 'done':
      return styles.badgeSuccess ?? ''
    case 'failed':
      return styles.badgeError ?? ''
    default:
      return styles.badgeInfo ?? ''
  }
}

export function ApplyProgressPanel({ progress, result }: ApplyProgressPanelProps) {
  const hasFailure = progress.some((entry) => entry.phase === 'failed')
  const partial =
    result?.fileResults.some((file) => file.outcome === 'applied') &&
    result.fileResults.some((file) => file.outcome === 'failed')

  return (
    <section className={styles.card} aria-label="Apply progress" role="status">
      <h3 className={styles.cardTitle}>Applying configuration</h3>
      <ul className={styles.list}>
        {progress.map((entry) => (
          <li key={entry.displayPath}>
            <span className={styles.mono}>{entry.displayPath}</span>
            <span className={phaseClass(entry.phase)}>{applyProgressLabel(entry.phase)}</span>
            {entry.message ? <p className={styles.muted}>{entry.message}</p> : null}
          </li>
        ))}
      </ul>
      {partial ? (
        <p className={styles.caveat} role="alert">
          Partial success — some files applied; failed files were left unchanged. Review the diff
          before retrying.
        </p>
      ) : null}
      {hasFailure && !partial ? (
        <p className={styles.caveat} role="alert">
          Apply failed — no files were changed.
        </p>
      ) : null}
      {!hasFailure && progress.every((entry) => entry.phase === 'done') ? (
        <p className={styles.muted}>All selected files verified.</p>
      ) : null}
    </section>
  )
}
