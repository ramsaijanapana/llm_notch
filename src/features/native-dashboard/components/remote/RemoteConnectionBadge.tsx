import type { RemoteConnectionState } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { remoteConnectionBadgeTone, remoteConnectionStateLabel } from '../../utils/remoteLabels'

const BADGE_CLASS = {
  info: styles.badgeInfo,
  warning: styles.badgeWarning,
  error: styles.badgeError,
  success: styles.badgeSuccess,
} as const

type RemoteConnectionBadgeProps = {
  state: RemoteConnectionState
  detail?: string | undefined
}

export function RemoteConnectionBadge({ state, detail }: RemoteConnectionBadgeProps) {
  const tone = remoteConnectionBadgeTone(state)
  const label = remoteConnectionStateLabel(state)

  return (
    <div>
      <p className={styles.muted}>
        Connection: <span className={BADGE_CLASS[tone]}>{label}</span>
      </p>
      {detail ? (
        <p className={styles.caveat} role="status">
          {detail}
        </p>
      ) : null}
    </div>
  )
}
