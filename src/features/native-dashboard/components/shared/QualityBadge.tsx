import type { MetricQuality } from '../../../../native/contracts'
import { attributionQualityLabel } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import { ioQualityLabel, metricAvailabilityLabel } from '../../utils/formatters'

type QualityBadgeProps = {
  quality: MetricQuality
}

export function QualityBadge({ quality }: QualityBadgeProps) {
  const attributionClass =
    quality.attribution === 'exact'
      ? styles.badgeSuccess
      : quality.attribution === 'unknown'
        ? styles.badgeError
        : styles.badgeWarning

  return (
    <section className={styles.actions} aria-label="Metric quality labels">
      <span className={attributionClass}>
        Attribution: {attributionQualityLabel(quality.attribution)}
      </span>
      <span className={styles.badgeInfo}>CPU: {metricAvailabilityLabel(quality.cpu)}</span>
      <span className={styles.badgeInfo}>I/O: {ioQualityLabel(quality.io)}</span>
      {quality.reason ? <span className={styles.badgeWarning}>{quality.reason}</span> : null}
    </section>
  )
}
