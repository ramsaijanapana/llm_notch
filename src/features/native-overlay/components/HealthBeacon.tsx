import type { HealthBeaconTone } from '../types'
import styles from './overlay.module.css'

interface HealthBeaconProps {
  tone: HealthBeaconTone
  attentionCount: number
}

function beaconClass(tone: HealthBeaconTone): string {
  switch (tone) {
    case 'healthy':
      return styles.beaconHealthy ?? ''
    case 'attention':
      return styles.beaconAttention ?? ''
    case 'degraded':
      return styles.beaconDegraded ?? ''
    case 'error':
      return styles.beaconError ?? ''
  }
}

function beaconLabel(tone: HealthBeaconTone): string {
  switch (tone) {
    case 'healthy':
      return 'Healthy'
    case 'attention':
      return 'Attention needed'
    case 'degraded':
      return 'Degraded'
    case 'error':
      return 'Error'
  }
}

export function HealthBeacon({ tone, attentionCount }: HealthBeaconProps) {
  return (
    <div className={styles.beaconCluster} aria-hidden="true">
      <span className={`${styles.beacon} ${beaconClass(tone)}`} />
      <span className={styles.beaconCount}>{attentionCount}</span>
      <span className={styles.visuallyHidden}>
        {beaconLabel(tone)}. {attentionCount} sessions need attention.
      </span>
    </div>
  )
}
