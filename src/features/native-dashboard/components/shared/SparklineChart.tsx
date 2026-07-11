import styles from '../../styles/dashboard.module.css'
import type { MetricHistoryPoint } from '../../types/contracts'

type SparklineChartProps = {
  points: MetricHistoryPoint[]
  label: string
  unit?: string
  width?: number
  height?: number
  className?: string | undefined
  reducedMotion?: boolean | undefined
  domainStartMs?: number | undefined
  domainEndMs?: number | undefined
}

export function historyPointX(
  atMs: number,
  domainStartMs: number,
  domainEndMs: number,
  width: number,
): number {
  const duration = Math.max(1, domainEndMs - domainStartMs)
  return Math.max(0, Math.min(width, ((atMs - domainStartMs) / duration) * width))
}

export function SparklineChart({
  points,
  label,
  unit = '',
  width = 240,
  height = 48,
  className,
  reducedMotion = false,
  domainStartMs,
  domainEndMs,
}: SparklineChartProps) {
  if (points.length === 0) {
    return (
      <svg
        className={className ?? styles.chart}
        width={width}
        height={height}
        role="img"
        aria-label={`${label}: no data`}
      >
        <title>{label}: no data</title>
        <text x="8" y={height / 2} fill="currentColor" fontSize="10">
          No history
        </text>
      </svg>
    )
  }

  const values = points.map((point) => point.value)
  const min = Math.min(...values)
  const max = Math.max(...values)
  const range = max - min || 1
  const resolvedDomainStart = domainStartMs ?? points[0]?.atMs ?? 0
  const resolvedDomainEnd = domainEndMs ?? points.at(-1)?.atMs ?? resolvedDomainStart + 1

  const path = points
    .map((point, index) => {
      const x = historyPointX(point.atMs, resolvedDomainStart, resolvedDomainEnd, width)
      const y = height - ((point.value - min) / range) * (height - 8) - 4
      return `${index === 0 ? 'M' : 'L'}${x.toFixed(2)},${y.toFixed(2)}`
    })
    .join(' ')

  const latest = points[points.length - 1]?.value ?? 0

  return (
    <svg
      className={className ?? styles.chart}
      width={width}
      height={height}
      role="img"
      aria-label={`${label}: latest ${latest}${unit}`}
    >
      <title>
        {label}: latest {latest}
        {unit}
      </title>
      <path
        d={path}
        fill="none"
        stroke="var(--color-amber)"
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
        style={reducedMotion ? undefined : { transition: 'd 200ms ease' }}
      />
    </svg>
  )
}
