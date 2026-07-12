import { attributionQualityLabel } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type {
  MetricSeriesCoverage,
  MetricsHistoryRange,
  MetricsPanelProps,
} from '../../types/contracts'
import {
  formatBytes,
  formatBytesPerSec,
  formatPercent,
  formatRelativeTime,
  historyRangeLabel,
  metricAvailabilityLabel,
  quotaObservedSummary,
} from '../../utils/formatters'
import { EmptyState } from '../shared/EmptyState'
import { ErrorState } from '../shared/ErrorState'
import { LoadingState } from '../shared/LoadingState'
import { SparklineChart } from '../shared/SparklineChart'

const RANGES: MetricsHistoryRange[] = ['15m', '1h', '24h']

function coverageLabel(coverage: MetricSeriesCoverage, selectedRange: MetricsHistoryRange): string {
  const coveredMs =
    coverage.actualFirstMs !== undefined && coverage.actualLastMs !== undefined
      ? Math.max(0, coverage.actualLastMs - coverage.actualFirstMs)
      : 0
  const coveredMinutes = Math.round(coveredMs / 60_000)
  const covered =
    coveredMinutes >= 60
      ? `${(coveredMinutes / 60).toFixed(coveredMinutes % 60 === 0 ? 0 : 1)}h`
      : `${coveredMinutes}m`
  const statuses = [
    `${covered} of selected ${historyRangeLabel(selectedRange)}`,
    coverage.downsampled
      ? `downsampled ${coverage.totalPoints} to ${coverage.returnedPoints} points`
      : null,
    coverage.truncated ? 'truncated' : null,
  ].filter(Boolean)
  return statuses.join(' · ')
}

export function MetricsPanel({
  host,
  aggregate,
  agents,
  history,
  historyRange,
  onHistoryRangeChange,
  loadState = 'ready',
  warmingUp = false,
  historyLoadState = 'ready',
  historyError,
  disabledHistoryRanges = [],
  quotas = [],
  onRefreshQuotas,
  quotaRefreshState = 'idle',
  nowMs = Date.now(),
}: MetricsPanelProps & { nowMs?: number }) {
  if (loadState === 'loading') {
    return <LoadingState label="Loading metrics…" />
  }

  if (loadState === 'empty' || (!host && !aggregate)) {
    return (
      <EmptyState
        title="Metrics unavailable"
        description="Host and agent metrics will appear once sampling begins."
      />
    )
  }

  const agentEntries = Object.entries(agents)
  const quotaObserved = quotaObservedSummary(quotas, nowMs)

  return (
    <div className={styles.panelGrid}>
      <fieldset className={`${styles.actions} ${styles.fieldsetReset}`}>
        <legend className="sr-only">History range</legend>
        {RANGES.map((range) => (
          <button
            key={range}
            type="button"
            className={range === historyRange ? styles.buttonPrimary : styles.button}
            aria-pressed={range === historyRange}
            disabled={disabledHistoryRanges.includes(range)}
            title={
              disabledHistoryRanges.includes(range)
                ? 'Unavailable with the configured retention window'
                : undefined
            }
            onClick={() => onHistoryRangeChange(range)}
          >
            {historyRangeLabel(range)}
          </button>
        ))}
      </fieldset>
      {warmingUp ? (
        <p className={styles.caveat} role="status">
          Metrics are warming up — values may be incomplete for the first sampling window.
        </p>
      ) : null}
      <div className={styles.cardsRow}>
        <article className={styles.card}>
          <h3 className={styles.cardTitle}>Host</h3>
          {host ? (
            <>
              <p className={styles.mono}>CPU {formatPercent(host.cpuHostPercent)}</p>
              <p className={styles.mono}>
                Memory {formatBytes(host.usedMemoryBytes)} / {formatBytes(host.totalMemoryBytes)}
              </p>
              <p className={styles.mono}>Processes {host.visibleProcessCount}</p>
              <SparklineChart
                points={history.hostCpu}
                label="Host CPU history"
                unit="%"
                domainStartMs={history.requestedStartMs}
                domainEndMs={history.requestedEndMs}
              />
            </>
          ) : (
            <p className={styles.muted}>Host metrics unavailable.</p>
          )}
        </article>

        <article className={styles.card}>
          <h3 className={styles.cardTitle}>Aggregate agents</h3>
          {aggregate ? (
            <>
              <p className={styles.mono}>CPU {formatPercent(aggregate.cpuCorePercent)}</p>
              <p className={styles.mono}>RSS {formatBytes(aggregate.rssBytes)}</p>
              <p className={styles.mono}>
                Active {aggregate.activeSessions} · Attention {aggregate.attentionSessions}
              </p>
              <SparklineChart
                points={history.aggregateCpu}
                label="Aggregate CPU history"
                unit="%"
                className={styles.chartLarge}
                domainStartMs={history.requestedStartMs}
                domainEndMs={history.requestedEndMs}
              />
            </>
          ) : (
            <p className={styles.muted}>Aggregate metrics unavailable.</p>
          )}
        </article>
      </div>
      <section className={styles.card} aria-labelledby="quota-title">
        <div className={styles.cardHeaderRow}>
          <h3 id="quota-title" className={styles.cardTitle}>
            Service quotas
          </h3>
          <div className={styles.actions} style={{ marginTop: 0 }}>
            {quotaObserved ? (
              <span
                className={styles.muted}
                role="status"
                data-testid="quota-observed-status"
                style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--space-2)' }}
              >
                Updated {formatRelativeTime(quotaObserved.observedAtMs, nowMs)}
                <span
                  className={
                    quotaObserved.freshness === 'stale' ? styles.badgeWarning : styles.badgeSuccess
                  }
                >
                  {quotaObserved.freshness === 'stale' ? 'Stale' : 'Fresh'}
                </span>
              </span>
            ) : null}
            {onRefreshQuotas ? (
              <button
                type="button"
                className={styles.button}
                aria-busy={quotaRefreshState === 'loading'}
                disabled={quotaRefreshState === 'loading'}
                onClick={onRefreshQuotas}
              >
                {quotaRefreshState === 'loading' ? 'Refreshing…' : 'Refresh quotas'}
              </button>
            ) : null}
          </div>
        </div>
        <div className={styles.cardsRow}>
          {quotas.map((quota) => (
            <article key={quota.service} className={styles.metricCell}>
              <span className={styles.metricLabel}>{quota.displayName}</span>
              {quota.availability === 'available' ? (
                <strong className={styles.metricValue}>
                  {quota.remaining ?? '—'} {quota.unit ?? ''} remaining
                </strong>
              ) : (
                <span className={styles.listSecondary}>
                  {quota.authentication === 'required'
                    ? (quota.message ?? 'Set provider credentials to enable quota probes.')
                    : (quota.message ?? 'Unavailable')}
                </span>
              )}
            </article>
          ))}
        </div>
      </section>
      <section className={styles.card}>
        <h3 className={styles.cardTitle}>Per-agent metrics</h3>
        {agentEntries.length === 0 ? (
          <p className={styles.muted}>No per-agent samples yet.</p>
        ) : (
          <table className={styles.table}>
            <thead>
              <tr>
                <th scope="col">Agent</th>
                <th scope="col">CPU</th>
                <th scope="col">RSS</th>
                <th scope="col">I/O</th>
                <th scope="col">Quality</th>
              </tr>
            </thead>
            <tbody>
              {agentEntries.map(([key, sample]) => (
                <tr key={key}>
                  <td>{key}</td>
                  <td>
                    {sample.quality.cpu === 'available'
                      ? formatPercent(sample.cpuCorePercent)
                      : metricAvailabilityLabel(sample.quality.cpu)}
                  </td>
                  <td>{formatBytes(sample.rssBytes)}</td>
                  <td>
                    {formatBytesPerSec(sample.readBytesPerSec)} /{' '}
                    {formatBytesPerSec(sample.writeBytesPerSec)}
                  </td>
                  <td>{attributionQualityLabel(sample.quality.attribution)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      <section className={styles.card}>
        <h3 className={styles.cardTitle}>History ({historyRangeLabel(historyRange)})</h3>
        {historyLoadState === 'loading' ? (
          <LoadingState label="Loading persisted history…" />
        ) : null}
        {historyLoadState === 'error' ? (
          <ErrorState message={historyError ?? 'Unable to load persisted metrics history.'} />
        ) : null}
        {historyLoadState === 'empty' ? (
          <EmptyState
            title="No history in this range"
            description="No persisted metric buckets were found for the selected range."
          />
        ) : null}
        {historyLoadState === 'ready' ? (
          <>
            <p className={styles.caveat} data-testid="host-history-coverage">
              Host coverage: {coverageLabel(history.hostCoverage, historyRange)}
            </p>
            <p className={styles.caveat} data-testid="aggregate-history-coverage">
              Aggregate coverage: {coverageLabel(history.aggregateCoverage, historyRange)}
            </p>
            <div className={styles.panelGrid}>
              {history.perAgent.map((agentHistory) => (
                <div key={agentHistory.sessionId}>
                  <p className={styles.listPrimary}>{agentHistory.label}</p>
                  <p className={styles.muted}>
                    Coverage: {coverageLabel(agentHistory.coverage, historyRange)}
                  </p>
                  <SparklineChart
                    points={agentHistory.cpu}
                    label={`${agentHistory.label} CPU`}
                    unit="%"
                    domainStartMs={history.requestedStartMs}
                    domainEndMs={history.requestedEndMs}
                  />
                  <SparklineChart
                    points={agentHistory.rss}
                    label={`${agentHistory.label} RSS`}
                    unit=" MB"
                    domainStartMs={history.requestedStartMs}
                    domainEndMs={history.requestedEndMs}
                  />
                </div>
              ))}
            </div>
          </>
        ) : null}
      </section>
      <p className={styles.caveat}>
        RSS reflects attributed resident memory and may under-count shared pages. On Windows, I/O
        counters may include all I/O rather than disk-only attribution.
      </p>
      <ul className={styles.unsupportedList}>
        <li>GPU utilization — unsupported</li>
        <li>Energy / power draw — unsupported</li>
        <li>Network throughput — unsupported</li>
      </ul>
    </div>
  )
}
