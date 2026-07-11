import type { AgentSession, AppSnapshot } from '../../../native/contracts'
import {
  formatAgentSource,
  formatAttentionKind,
  formatAttributionQuality,
  formatBytes,
  formatCpuPercent,
  formatIoQuality,
  formatMetricAvailability,
  formatRuntime,
  formatSessionStatus,
  formatThroughput,
} from '../model/overlay.helpers'
import {
  getConnectionBanner,
  getFooterMetrics,
  selectAttentionSessions,
  sortSessionsForPeek,
} from '../model/overlay.selectors'
import type { OverlayConnectionState, OverlayPlatform } from '../types'
import styles from './overlay.module.css'

export interface PeekPanelProps {
  platform: OverlayPlatform
  connectionState: OverlayConnectionState
  snapshot: AppSnapshot | undefined
  staleMessage?: string | undefined
  errorMessage?: string | undefined
  onOpenDashboard?: (() => void) | undefined
  onAcknowledge?: ((sessionId: string) => void) | undefined
}

function sessionStatusClass(session: AgentSession): string {
  if (session.attention !== 'none') {
    return styles.statusAttention ?? ''
  }
  if (session.status === 'running') {
    return styles.statusRunning ?? ''
  }
  if (session.status === 'failed') {
    return styles.statusFailed ?? ''
  }
  return ''
}

function SessionMetricRow({ session }: { session: AgentSession }) {
  const metric = session.latestMetric
  const cpuAvailability = metric?.quality.cpu ?? 'unavailable'

  return (
    <li className={styles.sessionRow} data-testid={`session-row-${session.id}`}>
      <div className={styles.sessionPrimary}>
        <span className={styles.sessionLabel}>{session.label}</span>
        <span className={styles.sessionSource}>{formatAgentSource(session.source)}</span>
      </div>
      <span className={`${styles.metricCell} ${sessionStatusClass(session)}`}>
        {session.attention !== 'none'
          ? formatAttentionKind(session.attention)
          : formatSessionStatus(session.status)}
      </span>
      <span className={styles.metricCell}>{formatRuntime(metric?.runtimeMs)}</span>
      <span className={styles.metricCell}>
        {formatCpuPercent(metric?.cpuCorePercent, cpuAvailability)}
      </span>
      <span className={styles.metricCell}>{formatBytes(metric?.rssBytes)}</span>
    </li>
  )
}

export function PeekPanel({
  platform,
  connectionState,
  snapshot,
  staleMessage,
  errorMessage,
  onOpenDashboard,
  onAcknowledge,
}: PeekPanelProps) {
  const sessions = snapshot?.sessions ?? []
  const attentionSessions = selectAttentionSessions(sessions)
  const orderedSessions = sortSessionsForPeek(sessions)
  const banner = getConnectionBanner(connectionState)
  const footer = getFooterMetrics(snapshot)

  const bannerText =
    connectionState === 'stale' && staleMessage
      ? staleMessage
      : (connectionState === 'ipcError' || connectionState === 'coreError') && errorMessage
        ? errorMessage
        : banner

  const showQualityNote = footer.attributionLabel !== undefined || footer.ioLabel !== undefined

  return (
    <section
      className={styles.peekPanel}
      data-platform={platform}
      data-testid="peek-panel"
      aria-label="Agent overlay peek view"
    >
      {bannerText ? (
        <p
          role={
            connectionState === 'ipcError' || connectionState === 'coreError' ? 'alert' : 'status'
          }
          className={`${styles.stateBanner} ${
            connectionState === 'ipcError' || connectionState === 'coreError'
              ? styles.stateBannerError
              : styles.stateBannerAlert
          }`}
          data-testid="connection-banner"
        >
          {bannerText}
        </p>
      ) : null}

      <div className={styles.peekBody}>
        {attentionSessions.length > 0 ? (
          <div className={styles.attentionSection} data-testid="attention-section">
            <p className={styles.sectionLabel}>Needs attention</p>
            {attentionSessions.map((session) => (
              <div key={session.id} className={styles.attentionItem}>
                <div className={styles.attentionCopy}>
                  <p className={styles.attentionTitle}>{session.label}</p>
                  <p className={styles.attentionMeta}>
                    {formatAgentSource(session.source)} · {formatAttentionKind(session.attention)}
                  </p>
                </div>
                {onAcknowledge ? (
                  <button
                    type="button"
                    className={styles.actionButton}
                    onClick={() => onAcknowledge(session.id)}
                    aria-label={`Acknowledge ${session.label}`}
                  >
                    Acknowledge
                  </button>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}

        {orderedSessions.length > 0 ? (
          <ul className={styles.sessionList} aria-label="Agent sessions">
            {orderedSessions.map((session) => (
              <SessionMetricRow key={session.id} session={session} />
            ))}
          </ul>
        ) : null}
      </div>

      <footer className={styles.peekFooter}>
        <ul className={styles.footerMetrics} aria-label="Combined resource usage">
          <li data-testid="footer-cpu">
            CPU {formatCpuPercent(footer.cpuCorePercent, footer.cpuAvailability)}
          </li>
          <li data-testid="footer-rss">RSS {formatBytes(footer.rssBytes)}</li>
          <li data-testid="footer-read">Read {formatThroughput(footer.readBytesPerSec)}</li>
          <li data-testid="footer-write">Write {formatThroughput(footer.writeBytesPerSec)}</li>
          <li data-testid="footer-processes">Processes {footer.processCount ?? '—'}</li>
        </ul>

        {showQualityNote ? (
          <p className={styles.qualityNote} data-testid="quality-note">
            {footer.attributionLabel
              ? `Attribution ${formatAttributionQuality(footer.attributionLabel)}`
              : null}
            {footer.attributionLabel && footer.ioLabel ? ' · ' : null}
            {footer.ioLabel ? `I/O ${formatIoQuality(footer.ioLabel)}` : null}
            {footer.cpuAvailability !== 'available'
              ? ` · CPU ${formatMetricAvailability(footer.cpuAvailability)}`
              : null}
          </p>
        ) : null}

        <div className={styles.footerActions}>
          {onOpenDashboard ? (
            <button
              type="button"
              className={`${styles.actionButton} ${styles.actionPrimary}`}
              onClick={onOpenDashboard}
            >
              Open dashboard
            </button>
          ) : null}
        </div>
      </footer>
    </section>
  )
}
