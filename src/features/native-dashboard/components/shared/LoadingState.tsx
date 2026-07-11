import styles from '../../styles/dashboard.module.css'

type LoadingStateProps = {
  label?: string
}

export function LoadingState({ label = 'Loading dashboard data…' }: LoadingStateProps) {
  return (
    <div className={styles.stateBox} role="status" aria-busy="true" aria-live="polite">
      <p className={styles.muted}>{label}</p>
    </div>
  )
}
