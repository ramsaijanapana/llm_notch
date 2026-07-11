import type { AppSnapshot } from '../../../native/contracts'
import { formatCpuPercent } from '../model/overlay.helpers'
import {
  compactAriaLabel,
  countAttentionSessions,
  deriveHealthBeaconTone,
  getCombinedCpuReading,
  selectCompactDots,
} from '../model/overlay.selectors'
import type { OverlayConnectionState, OverlayCpuSample, OverlayPlatform } from '../types'
import { CpuSparkline } from './CpuSparkline'
import { HealthBeacon } from './HealthBeacon'
import styles from './overlay.module.css'
import { SessionDot } from './SessionDot'

export interface CompactIslandProps {
  platform: OverlayPlatform
  connectionState: OverlayConnectionState
  snapshot: AppSnapshot | undefined
  cpuHistory: readonly OverlayCpuSample[]
  nowMs: number
  reducedMotion: boolean
}

export function CompactIsland({
  platform,
  connectionState,
  snapshot,
  cpuHistory,
  nowMs,
  reducedMotion,
}: CompactIslandProps) {
  const sessions = snapshot?.sessions ?? []
  const attentionCount = countAttentionSessions(sessions)
  const resourceAlertCount = snapshot?.resourceAlerts?.length ?? 0
  const { visible, overflowCount } = selectCompactDots(sessions)
  const cpuReading = getCombinedCpuReading(snapshot)
  const cpuLabel = formatCpuPercent(cpuReading.value, cpuReading.availability)
  const beaconTone = deriveHealthBeaconTone(
    connectionState,
    attentionCount,
    resourceAlertCount,
  )

  return (
    <section
      className={styles.compactIsland}
      data-platform={platform}
      data-testid="compact-island"
      aria-label={compactAriaLabel({
        attentionCount,
        sessionCount: sessions.length,
        cpuLabel,
        connectionState,
      })}
    >
      <HealthBeacon tone={beaconTone} attentionCount={attentionCount} />

      <div className={styles.dotRow} aria-hidden="true">
        {visible.map((session) => (
          <SessionDot key={session.id} session={session} />
        ))}
        {overflowCount > 0 ? (
          <span className={styles.overflowBadge} data-testid="session-overflow">
            +{overflowCount}
          </span>
        ) : null}
      </div>

      <div className={styles.cpuCluster}>
        <span className={styles.cpuValue} data-testid="compact-cpu">
          {cpuLabel}
        </span>
        <CpuSparkline
          history={cpuHistory}
          reading={cpuReading}
          nowMs={nowMs}
          reducedMotion={reducedMotion}
        />
      </div>
    </section>
  )
}
