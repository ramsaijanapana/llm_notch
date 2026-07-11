import styles from '../../styles/dashboard.module.css'

type EmptyStateProps = {
  title: string
  description: string
}

export function EmptyState({ title, description }: EmptyStateProps) {
  return (
    <div className={styles.stateBox} role="status">
      <h3 className={styles.stateTitle}>{title}</h3>
      <p className={styles.muted}>{description}</p>
    </div>
  )
}
