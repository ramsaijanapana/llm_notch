import { formatCpuPercent, summarizeSparkline } from '../model/overlay.helpers'
import type { CombinedCpuReading } from '../model/overlay.selectors'
import {
  buildSparklinePoints,
  SPARKLINE_WINDOW_MS,
  sparklinePolyline,
} from '../model/overlay.selectors'
import type { OverlayCpuSample } from '../types'
import styles from './overlay.module.css'

interface CpuSparklineProps {
  history: readonly OverlayCpuSample[]
  reading: CombinedCpuReading
  nowMs: number
  reducedMotion: boolean
}

export function CpuSparkline({ history, reading, nowMs, reducedMotion }: CpuSparklineProps) {
  const points = buildSparklinePoints(history, nowMs)
  const values = points.length
    ? history
        .filter((sample) => sample.atMs >= nowMs - SPARKLINE_WINDOW_MS && sample.atMs <= nowMs)
        .map((sample) => sample.cpuCorePercent)
    : []

  const polyline = sparklinePolyline(points)
  const ariaLabel = summarizeSparkline(values)
  const cpuLabel = formatCpuPercent(reading.value, reading.availability)

  return (
    <svg
      className={styles.sparkline}
      viewBox="0 0 56 18"
      role="img"
      aria-label={ariaLabel}
      data-testid="cpu-sparkline"
    >
      <title>{ariaLabel}</title>
      {polyline ? (
        <polyline
          className={styles.sparklineLine}
          points={polyline}
          vectorEffect="non-scaling-stroke"
        />
      ) : (
        <line
          className={`${styles.sparklineLine} ${styles.sparklineEmpty}`}
          x1="0"
          y1="9"
          x2="56"
          y2="9"
        />
      )}
      <text className={styles.visuallyHidden}>
        {reducedMotion ? 'Static CPU sparkline.' : ariaLabel} Current reading {cpuLabel}.
      </text>
    </svg>
  )
}
