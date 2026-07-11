import styles from '../../styles/dashboard.module.css'

type ErrorStateProps = {
  title?: string
  message: string
}

export function ErrorState({ title = 'Something went wrong', message }: ErrorStateProps) {
  return (
    <div className={styles.stateBox} role="alert">
      <h3 className={styles.stateTitle}>{title}</h3>
      <p className={styles.muted}>{message}</p>
    </div>
  )
}
