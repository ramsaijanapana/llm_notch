import { useState } from 'react'
import styles from '../../styles/dashboard.module.css'
import type { RemoteHostConfigInput, RemotePanelProps } from '../../types/contracts'
import { formatRelativeTime } from '../../utils/formatters'
import {
  remoteBackendGuidance,
  remoteDeploymentStepLabel,
} from '../../utils/remoteLabels'
import { summarizeRemoteIngestByHost } from '../../utils/remoteSessionStats'
import { EmptyState } from '../shared/EmptyState'
import { LoadingState } from '../shared/LoadingState'
import { RemoteConnectionBadge } from './RemoteConnectionBadge'

const DEFAULT_HOST_FORM: RemoteHostConfigInput = {
  id: '',
  destination: '',
  port: null,
  identityFile: null,
  hostKeyPolicy: 'strict',
  connectTimeoutSeconds: 10,
}

export function RemotePanel({
  hosts,
  sessions = [],
  backendStatus,
  pendingDeployPlan,
  pendingDeployResult,
  deployBusy = false,
  loadState = 'ready',
  lifecycleActionsAvailable = false,
  hostConfigActionsAvailable = false,
  onPlanDeploy,
  onExecuteDeploy,
  onStartRelay,
  onStopRelay,
  onDismissPlan,
  onAddHost,
  onRemoveHost,
  nowMs = Date.now(),
}: RemotePanelProps & { nowMs?: number }) {
  const [hostForm, setHostForm] = useState<RemoteHostConfigInput>(DEFAULT_HOST_FORM)
  const backendUnavailable = backendStatus.availability === 'unavailable'
  const backendGuidance = remoteBackendGuidance(
    backendStatus.availability,
    backendStatus.message,
  )
  const ingestByHost = summarizeRemoteIngestByHost(sessions)

  if (loadState === 'loading') {
    return <LoadingState label="Loading remote hosts…" />
  }

  const submitHost = () => {
    if (!onAddHost || !hostForm.id.trim() || !hostForm.destination.trim()) {
      return
    }
    onAddHost({
      ...hostForm,
      id: hostForm.id.trim(),
      destination: hostForm.destination.trim(),
      port: hostForm.port ?? null,
      identityFile: hostForm.identityFile?.trim() ? hostForm.identityFile.trim() : null,
    })
    setHostForm(DEFAULT_HOST_FORM)
  }

  return (
    <div className={styles.panelGrid}>
      <section className={styles.card} aria-labelledby="remote-backend-title">
        <h2 id="remote-backend-title" className={styles.cardTitle}>
          SSH relay backend
        </h2>
        <p className={styles.muted}>
          Status:{' '}
          <span className={backendUnavailable ? styles.badgeWarning : styles.badgeSuccess}>
            {backendUnavailable ? 'Unavailable' : 'Available'}
          </span>
        </p>
        {backendGuidance ? (
          <p className={styles.caveat} role="status">
            {backendGuidance}
          </p>
        ) : null}
        <dl className={styles.capabilityGrid}>
          <div>
            <dt className={styles.metricLabel}>OpenSSH</dt>
            <dd className={styles.listSecondary}>
              {backendStatus.sshExecutablePresent == null
                ? 'Not probed'
                : backendStatus.sshExecutablePresent
                  ? 'Detected'
                  : 'Missing'}
            </dd>
          </div>
          <div>
            <dt className={styles.metricLabel}>Relay binary</dt>
            <dd className={styles.listSecondary}>
              {backendStatus.relayBinaryPresent == null
                ? 'Not probed'
                : backendStatus.relayBinaryPresent
                  ? 'Present'
                  : 'Missing'}
            </dd>
          </div>
        </dl>
      </section>

      <section className={styles.card} aria-labelledby="remote-add-host-title">
        <h2 id="remote-add-host-title" className={styles.cardTitle}>
          Add SSH host
        </h2>
        <p className={styles.muted}>
          Host entries are saved locally. Relay start, stop, and deploy planning still require the
          SSH relay backend.
        </p>
        <div className={styles.field}>
          <label htmlFor="remote-host-id">Host id</label>
          <input
            id="remote-host-id"
            className={styles.input}
            value={hostForm.id}
            disabled={!hostConfigActionsAvailable}
            onChange={(event) =>
              setHostForm((current) => ({ ...current, id: event.target.value }))
            }
            placeholder="dev-box"
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="remote-host-destination">Destination</label>
          <input
            id="remote-host-destination"
            className={styles.input}
            value={hostForm.destination}
            disabled={!hostConfigActionsAvailable}
            onChange={(event) =>
              setHostForm((current) => ({ ...current, destination: event.target.value }))
            }
            placeholder="dev@example.internal"
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="remote-host-port">Port</label>
          <input
            id="remote-host-port"
            className={styles.input}
            type="number"
            min={1}
            max={65535}
            value={hostForm.port ?? ''}
            disabled={!hostConfigActionsAvailable}
            onChange={(event) =>
              setHostForm((current) => ({
                ...current,
                port: event.target.value ? Number(event.target.value) : null,
              }))
            }
            placeholder="22"
          />
        </div>
        <div className={styles.field}>
          <label htmlFor="remote-host-key-policy">Host key policy</label>
          <select
            id="remote-host-key-policy"
            className={styles.select}
            value={hostForm.hostKeyPolicy}
            disabled={!hostConfigActionsAvailable}
            onChange={(event) =>
              setHostForm((current) => ({
                ...current,
                hostKeyPolicy: event.target.value as RemoteHostConfigInput['hostKeyPolicy'],
              }))
            }
          >
            <option value="strict">Strict</option>
            <option value="acceptNew">Accept new</option>
          </select>
        </div>
        <div className={styles.actions}>
          <button
            type="button"
            className={styles.buttonPrimary}
            disabled={
              !hostConfigActionsAvailable || !hostForm.id.trim() || !hostForm.destination.trim()
            }
            onClick={submitHost}
          >
            Save host
          </button>
        </div>
        {!hostConfigActionsAvailable ? (
          <p className={styles.caveat}>Host configuration is read-only in this preview mode.</p>
        ) : null}
      </section>

      {hosts.length === 0 ? (
        <EmptyState
          title="No remote hosts configured"
          description="Saved SSH host entries will appear here. Add a destination above to configure your first host."
        />
      ) : (
        <section aria-labelledby="remote-hosts-title">
          <h2 id="remote-hosts-title" className={styles.cardTitle}>
            Configured hosts
          </h2>
          <div className={styles.cardsRow}>
            {hosts.map((host) => {
              const { config } = host
              const actionsDisabled = backendUnavailable || !lifecycleActionsAvailable
              const ingest = ingestByHost[config.id]

              return (
                <article
                  key={config.id}
                  className={styles.card}
                  aria-label={`${config.id} remote host`}
                >
                  <h3 className={styles.cardTitle}>{config.id}</h3>
                  <p className={styles.muted}>
                    Destination: {config.destination}
                    {config.port ? `:${config.port}` : ''}
                  </p>
                  <p className={styles.muted}>
                    Host key policy: {config.hostKeyPolicy === 'strict' ? 'Strict' : 'Accept new'}
                  </p>
                  <p className={styles.muted}>
                    Last connected:{' '}
                    {host.lastConnectedAtMs
                      ? formatRelativeTime(host.lastConnectedAtMs, nowMs)
                      : 'Never'}
                  </p>
                  <dl className={styles.capabilityGrid}>
                    <div>
                      <dt className={styles.metricLabel}>Ingested sessions</dt>
                      <dd className={styles.listSecondary}>{ingest?.totalSessions ?? 0}</dd>
                    </div>
                    <div>
                      <dt className={styles.metricLabel}>Active ingested</dt>
                      <dd className={styles.listSecondary}>{ingest?.activeSessions ?? 0}</dd>
                    </div>
                    <div>
                      <dt className={styles.metricLabel}>Last ingested event</dt>
                      <dd className={styles.listSecondary}>
                        {ingest?.lastEventAtMs
                          ? formatRelativeTime(ingest.lastEventAtMs, nowMs)
                          : 'None'}
                      </dd>
                    </div>
                  </dl>
                  <RemoteConnectionBadge
                    state={host.connectionState}
                    detail={host.message ?? undefined}
                  />
                  <div className={styles.actions}>
                    <button
                      type="button"
                      className={styles.button}
                      disabled={actionsDisabled}
                      onClick={() => onPlanDeploy(config.id)}
                    >
                      Preview deploy plan
                    </button>
                    <button
                      type="button"
                      className={styles.buttonPrimary}
                      disabled={actionsDisabled}
                      onClick={() => onStartRelay(config.id)}
                    >
                      Start relay
                    </button>
                    <button
                      type="button"
                      className={styles.buttonDanger}
                      disabled={actionsDisabled}
                      onClick={() => onStopRelay(config.id)}
                    >
                      Stop relay
                    </button>
                    {hostConfigActionsAvailable && onRemoveHost ? (
                      <button
                        type="button"
                        className={styles.buttonDanger}
                        onClick={() => onRemoveHost(config.id)}
                      >
                        Remove host
                      </button>
                    ) : null}
                  </div>
                  {actionsDisabled ? (
                    <p className={styles.caveat}>
                      Lifecycle actions are disabled until the SSH relay backend is available in
                      this build.
                    </p>
                  ) : null}
                </article>
              )
            })}
          </div>
        </section>
      )}

      {pendingDeployPlan ? (
        <section className={styles.card} aria-labelledby="remote-plan-title">
          <h3 id="remote-plan-title" className={styles.cardTitle}>
            Deployment plan preview
          </h3>
          <p className={styles.muted}>Host: {pendingDeployPlan.hostId}</p>
          {pendingDeployPlan.availability === 'unavailable' ? (
            <p className={styles.caveat} role="status">
              {pendingDeployPlan.message ?? 'Deployment planning is unavailable.'}
            </p>
          ) : (
            <ol className={styles.list}>
              {pendingDeployPlan.steps.map((step, index) => (
                <li key={`${pendingDeployPlan.hostId}-${index}`}>
                  {remoteDeploymentStepLabel(step)}
                </li>
              ))}
            </ol>
          )}
          {pendingDeployResult ? (
            <p className={styles.muted} role="status">
              {pendingDeployResult.message ?? 'Deployment completed.'}
            </p>
          ) : null}
          <div className={styles.actions}>
            {pendingDeployPlan.availability === 'available' && onExecuteDeploy ? (
              <button
                type="button"
                className={styles.buttonPrimary}
                disabled={deployBusy || backendUnavailable || !lifecycleActionsAvailable}
                onClick={() => onExecuteDeploy(pendingDeployPlan.hostId)}
              >
                {deployBusy ? 'Deploying…' : 'Execute deploy'}
              </button>
            ) : null}
            <button type="button" className={styles.button} onClick={onDismissPlan}>
              Close preview
            </button>
          </div>
        </section>
      ) : null}
    </div>
  )
}
