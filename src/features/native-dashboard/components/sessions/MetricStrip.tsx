import type { MetricSample } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import {
  formatBytes,
  formatBytesPerSec,
  formatDurationMs,
  formatPercent,
} from '../../utils/formatters'
import { QualityBadge } from '../shared/QualityBadge'

type MetricStripProps = {
  metric?: MetricSample | undefined
}

export function MetricStrip({ metric }: MetricStripProps) {
  if (!metric) {
    return (
      <fieldset className={`${styles.metricStrip} ${styles.fieldsetReset}`}>
        <legend className="sr-only">Session metrics unavailable</legend>
        <p className={styles.muted}>Metrics unavailable for this session.</p>
      </fieldset>
    )
  }

  return (
    <>
      <fieldset className={`${styles.metricStrip} ${styles.fieldsetReset}`}>
        <legend className="sr-only">Current session metrics</legend>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>CPU (core)</span>
          <span className={styles.metricValue}>{formatPercent(metric.cpuCorePercent)}</span>
        </div>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>CPU (host)</span>
          <span className={styles.metricValue}>{formatPercent(metric.cpuHostPercent)}</span>
        </div>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>RSS</span>
          <span className={styles.metricValue}>{formatBytes(metric.rssBytes)}</span>
        </div>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>Runtime</span>
          <span className={styles.metricValue}>{formatDurationMs(metric.runtimeMs)}</span>
        </div>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>Processes</span>
          <span className={styles.metricValue}>{metric.processCount}</span>
        </div>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>Read I/O</span>
          <span className={styles.metricValue}>{formatBytesPerSec(metric.readBytesPerSec)}</span>
        </div>
        <div className={styles.metricCell}>
          <span className={styles.metricLabel}>Write I/O</span>
          <span className={styles.metricValue}>{formatBytesPerSec(metric.writeBytesPerSec)}</span>
        </div>
      </fieldset>
      <QualityBadge quality={metric.quality} />
    </>
  )
}
