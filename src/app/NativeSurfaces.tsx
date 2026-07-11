import { useEffect, useMemo, useState } from 'react'
import {
  agentLabel,
  type DashboardLoadState,
  DashboardShell,
  type DashboardTab,
  type IntegrationCardState,
  type IntegrationDiffPreview,
  type IntegrationHealth,
  IntegrationsPanel,
  type MetricSeriesCoverage,
  type MetricsHistoryBundle,
  type MetricsHistoryRange,
  MetricsPanel,
  OnboardingFlow,
  type OnboardingIntegrationChoice,
  type OnboardingStep,
  SessionsPanel,
  SettingsPanel,
} from '../features/native-dashboard'
import {
  type OverlayConnectionState,
  type OverlayCpuSample,
  type OverlayMode,
  type OverlayPlatform,
  OverlayShell,
} from '../features/native-overlay'
import type { AgentSession, AgentSource, AppSnapshot, PublicSettings } from '../native/contracts.ts'
import type { ConnectorUserStatus, IntegrationHealthReport, NativeHistoryResponse } from '../native/types.ts'
import { useNativeState } from '../state/NativeStateProvider.tsx'

const SHORTCUT_LABEL = 'CmdOrCtrl+Shift+Space'
const ONBOARDING_KEY = 'llm-notch:onboarding-complete:v1'

const EMPTY_SETTINGS: PublicSettings = {
  overlayEnabled: true,
  autostartEnabled: false,
  reducedMotion: false,
  samplingIntervalMs: 1_000,
  showOverFullscreen: false,
  historyRetentionHours: 24,
}

function currentPlatform(): OverlayPlatform {
  if (typeof navigator !== 'undefined' && /windows/i.test(navigator.userAgent)) {
    return 'windows'
  }
  return 'macos'
}

function deriveOverlayConnection(
  connection: ReturnType<typeof useNativeState>['state']['connection'],
  snapshot: AppSnapshot | undefined,
): OverlayConnectionState {
  if (connection === 'disconnected') return 'ipcError'
  if (connection === 'incompatible-protocol') return 'coreError'
  if (connection === 'resyncing') return 'stale'
  if (connection === 'loading') return 'warmingUp'
  if (!snapshot || snapshot.sessions.length === 0) return 'empty'
  if (!snapshot.aggregate) return 'metricsUnavailable'
  if (snapshot.aggregate.quality.cpu === 'warmingUp') return 'warmingUp'
  return 'live'
}

function useCurrentSnapshot(): AppSnapshot | undefined {
  const { state } = useNativeState()
  return useMemo(() => {
    if (!state.snapshot) return undefined
    const host = state.metrics?.host ?? state.snapshot.host
    const aggregate = state.metrics?.aggregate ?? state.snapshot.aggregate
    return {
      ...state.snapshot,
      ...(host ? { host } : {}),
      ...(aggregate ? { aggregate } : {}),
      sessions: state.sessionOrder
        .map((sessionId) => state.sessions[sessionId])
        .filter((session) => session !== undefined),
      settings: state.settings ?? state.snapshot.settings,
      adapters: state.adapters,
      capturedAtMs: state.metrics?.host.atMs ?? state.snapshot.capturedAtMs,
    }
  }, [
    state.adapters,
    state.metrics,
    state.sessionOrder,
    state.sessions,
    state.settings,
    state.snapshot,
  ])
}

export function NativeOverlaySurface() {
  const { state, client, prefersReducedMotion } = useNativeState()
  const snapshot = useCurrentSnapshot()
  const [mode, setMode] = useState<OverlayMode>('compact')
  const [cpuHistory, setCpuHistory] = useState<OverlayCpuSample[]>([])
  const aggregate = snapshot?.aggregate

  useEffect(() => {
    if (!aggregate) return
    setCpuHistory((history) => {
      const cutoff = aggregate.atMs - 30_000
      return [
        ...history.filter((sample) => sample.atMs >= cutoff),
        { atMs: aggregate.atMs, cpuCorePercent: aggregate.cpuCorePercent },
      ].slice(-120)
    })
  }, [aggregate])

  const changeMode = (nextMode: OverlayMode) => {
    setMode(nextMode)
    void client.setOverlayMode(nextMode === 'compact' ? 'collapsed' : 'peek').catch(() => {})
  }

  return (
    <div
      data-native-overlay-container
      onPointerEnter={() => changeMode('peek')}
      onPointerLeave={() => changeMode('compact')}
      onFocusCapture={() => changeMode('peek')}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) changeMode('compact')
      }}
    >
      <OverlayShell
        mode={mode}
        renderContext={state.clientMode === 'preview' ? 'preview' : 'native'}
        platform={currentPlatform()}
        reducedMotion={prefersReducedMotion}
        connectionState={deriveOverlayConnection(state.connection, snapshot)}
        snapshot={snapshot}
        cpuHistory={cpuHistory}
        staleMessage={state.resyncReason ?? undefined}
        errorMessage={state.errorMessage ?? undefined}
        onOpenDashboard={() => {
          void client.openDashboard().catch(() => {})
        }}
        onAcknowledge={(sessionId) => {
          void client.acknowledgeLocalAttention(sessionId).catch(() => {})
        }}
      />
    </div>
  )
}

function readOnboardingComplete(): boolean {
  try {
    return localStorage.getItem(ONBOARDING_KEY) === 'true'
  } catch {
    return false
  }
}

function saveOnboardingComplete() {
  try {
    localStorage.setItem(ONBOARDING_KEY, 'true')
  } catch {
    // Storage can be unavailable in hardened previews; completion remains local
    // to this renderer session in that case.
  }
}

function filterHistory(
  history: MetricsHistoryBundle,
  range: MetricsHistoryRange,
): MetricsHistoryBundle {
  const duration = range === '15m' ? 15 * 60_000 : range === '1h' ? 60 * 60_000 : 24 * 60 * 60_000
  const requestedEndMs = Date.now()
  const requestedStartMs = requestedEndMs - duration
  const keep = <T extends { atMs: number }>(points: T[]) =>
    points.filter((point) => point.atMs >= requestedStartMs && point.atMs <= requestedEndMs)
  const hostCpu = keep(history.hostCpu)
  const aggregateCpu = keep(history.aggregateCpu)
  const aggregateRss = keep(history.aggregateRss)
  return {
    requestedStartMs,
    requestedEndMs,
    hostCpu,
    aggregateCpu,
    aggregateRss,
    hostCoverage: coverageFor(hostCpu, requestedStartMs, requestedEndMs),
    aggregateCoverage: coverageFor(aggregateCpu, requestedStartMs, requestedEndMs),
    perAgent: history.perAgent.map((agent) => ({
      ...agent,
      cpu: keep(agent.cpu),
      rss: keep(agent.rss),
      coverage: coverageFor(keep(agent.cpu), requestedStartMs, requestedEndMs),
    })),
  }
}

function coverageFor(
  points: Array<{ atMs: number }>,
  requestedStartMs: number,
  requestedEndMs: number,
  metadata?: Partial<MetricSeriesCoverage>,
): MetricSeriesCoverage {
  const first = points[0]
  const last = points.at(-1)
  return {
    requestedStartMs,
    requestedEndMs,
    ...(first ? { actualFirstMs: first.atMs } : {}),
    ...(last ? { actualLastMs: last.atMs } : {}),
    totalPoints: metadata?.totalPoints ?? points.length,
    returnedPoints: metadata?.returnedPoints ?? points.length,
    downsampled: metadata?.downsampled ?? false,
    truncated: metadata?.truncated ?? false,
  }
}

function emptyHistory(requestedStartMs: number, requestedEndMs: number): MetricsHistoryBundle {
  const coverage = coverageFor([], requestedStartMs, requestedEndMs)
  return {
    requestedStartMs,
    requestedEndMs,
    hostCpu: [],
    aggregateCpu: [],
    aggregateRss: [],
    hostCoverage: coverage,
    aggregateCoverage: coverage,
    perAgent: [],
  }
}

export function persistedHistoryBundle(
  response: NativeHistoryResponse,
  sessions: AgentSession[],
): MetricsHistoryBundle {
  const seriesCoverage = (
    series: NativeHistoryResponse['host'] | NativeHistoryResponse['agents'][number],
  ) =>
    coverageFor(series.points, response.sinceMs, response.endMs, {
      totalPoints: series.totalPoints,
      returnedPoints: series.returnedPoints,
      downsampled: series.downsampled,
      truncated: series.truncated,
    })
  return {
    requestedStartMs: response.sinceMs,
    requestedEndMs: response.endMs,
    hostCpu: response.host.points.map((point) => ({
      atMs: point.atMs,
      value: point.cpuHostPercent,
    })),
    aggregateCpu: response.aggregate.points.map((point) => ({
      atMs: point.atMs,
      value: point.cpuCorePercent,
    })),
    aggregateRss: response.aggregate.points.map((point) => ({
      atMs: point.atMs,
      value: point.rssBytes,
    })),
    hostCoverage: seriesCoverage(response.host),
    aggregateCoverage: seriesCoverage(response.aggregate),
    perAgent: response.agents.flatMap((series) => {
      const session = sessions.find((entry) => entry.id === series.sessionId)
      if (!session) return []
      return [
        {
          sessionId: session.id,
          source: session.source,
          label: `${agentLabel(session.source)} — ${session.label}`,
          cpu: series.points.map((point) => ({
            atMs: point.atMs,
            value: point.cpuCorePercent,
          })),
          rss: series.points.map((point) => ({
            atMs: point.atMs,
            value: point.rssBytes / 1024 ** 2,
          })),
          coverage: seriesCoverage(series),
        },
      ]
    }),
  }
}

function mapConnectorStatusToDashboardHealth(
  status: ConnectorUserStatus | undefined,
): IntegrationHealth {
  switch (status) {
    case 'connected':
      return 'healthy'
    case 'notFound':
      return 'unknown'
    case 'error':
      return 'offline'
    case undefined:
      return 'unknown'
    default:
      return 'degraded'
  }
}

function templatePath(source: AgentSource): string {
  switch (source) {
    case 'cursor':
      return 'integrations/cursor/hooks.json.template'
    case 'claudeCode':
      return 'integrations/claude-code/settings.hooks.template.json'
    case 'codex':
      return 'integrations/codex/hooks.json.template'
    case 'generic':
      return 'integrations/generic/emit-examples.sh'
    case 'unknown':
      return 'No supported template'
  }
}

export function NativeDashboardSurface() {
  const { state, dispatch, client, prefersReducedMotion } = useNativeState()
  const fullscreenPreferenceSupported = currentPlatform() !== 'windows'
  const snapshot = useCurrentSnapshot()
  const settings = state.settings ?? EMPTY_SETTINGS
  const sessions = state.sessionOrder
    .map((sessionId) => state.sessions[sessionId])
    .filter((session) => session !== undefined)
  const [activeTab, setActiveTab] = useState<DashboardTab>('sessions')
  const [liveHistory, setLiveHistory] = useState<MetricsHistoryBundle>(() => {
    const end = Date.now()
    return emptyHistory(end - 15 * 60_000, end)
  })
  const [health, setHealth] = useState<IntegrationHealthReport | null>(null)
  const [pendingDiff, setPendingDiff] = useState<IntegrationDiffPreview>()
  const [actionError, setActionError] = useState<string>()
  const [purgeConfirmOpen, setPurgeConfirmOpen] = useState(false)
  const [onboardingOpen, setOnboardingOpen] = useState(() => !readOnboardingComplete())
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep>(0)
  const [onboardingIntegration, setOnboardingIntegration] =
    useState<OnboardingIntegrationChoice>('none')

  useEffect(() => {
    if (!state.metrics) return
    const metrics = state.metrics
    setLiveHistory((current) => {
      const perSession = new Map(current.perAgent.map((entry) => [entry.sessionId, entry]))
      for (const [sessionId, sample] of Object.entries(metrics.agents)) {
        const session = state.sessions[sessionId]
        if (!session) continue
        const existing = perSession.get(sessionId) ?? {
          sessionId,
          source: session.source,
          label: `${agentLabel(session.source)} — ${session.label}`,
          cpu: [],
          rss: [],
          coverage: coverageFor([], current.requestedStartMs, current.requestedEndMs),
        }
        const cpu = [...existing.cpu, { atMs: sample.atMs, value: sample.cpuCorePercent }].slice(
          -900,
        )
        const rss = [
          ...existing.rss,
          { atMs: sample.atMs, value: sample.rssBytes / 1024 ** 2 },
        ].slice(-900)
        perSession.set(sessionId, {
          ...existing,
          label: `${agentLabel(session.source)} — ${session.label}`,
          cpu,
          rss,
          coverage: coverageFor(cpu, current.requestedStartMs, current.requestedEndMs),
        })
      }
      const hostCpu = [
        ...current.hostCpu,
        { atMs: metrics.host.atMs, value: metrics.host.cpuHostPercent },
      ].slice(-900)
      const aggregateCpu = [
        ...current.aggregateCpu,
        { atMs: metrics.aggregate.atMs, value: metrics.aggregate.cpuCorePercent },
      ].slice(-900)
      const aggregateRss = [
        ...current.aggregateRss,
        { atMs: metrics.aggregate.atMs, value: metrics.aggregate.rssBytes },
      ].slice(-900)
      return {
        ...current,
        hostCpu,
        aggregateCpu,
        aggregateRss,
        hostCoverage: coverageFor(hostCpu, current.requestedStartMs, current.requestedEndMs),
        aggregateCoverage: coverageFor(
          aggregateCpu,
          current.requestedStartMs,
          current.requestedEndMs,
        ),
        perAgent: [...perSession.values()],
      }
    })
  }, [state.metrics, state.sessions])

  useEffect(() => {
    let cancelled = false
    dispatch({ type: 'SET_DISPLAYS_LOADING' })
    void client
      .listDisplays()
      .then((displays) => {
        if (!cancelled) dispatch({ type: 'SET_DISPLAYS', displays })
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          dispatch({
            type: 'SET_DISPLAYS_ERROR',
            message: error instanceof Error ? error.message : 'Display enumeration failed',
          })
        }
      })
    return () => {
      cancelled = true
    }
  }, [client, dispatch])

  useEffect(() => {
    if (state.historyRange === '15m') return
    let cancelled = false
    const range = state.historyRange
    dispatch({ type: 'SET_HISTORY_LOADING', range })
    void client
      .getHistory(range)
      .then((history) => {
        if (!cancelled) dispatch({ type: 'SET_HISTORY', history })
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          dispatch({
            type: 'SET_HISTORY_ERROR',
            range,
            message: error instanceof Error ? error.message : 'Persisted history load failed',
          })
        }
      })
    return () => {
      cancelled = true
    }
  }, [client, dispatch, state.historyRange])

  useEffect(() => {
    let cancelled = false
    void client
      .getIntegrationHealth()
      .then((report) => {
        if (!cancelled) setHealth(report)
      })
      .catch((error: unknown) => {
        if (!cancelled)
          setActionError(error instanceof Error ? error.message : 'Health check failed')
      })
    return () => {
      cancelled = true
    }
  }, [client])

  const loadState: DashboardLoadState =
    state.connection === 'loading' || state.connection === 'resyncing'
      ? 'loading'
      : state.connection === 'disconnected' || state.connection === 'incompatible-protocol'
        ? 'error'
        : 'ready'

  const displays = state.displays
  const selectedDisplayMissing =
    settings.selectedDisplay !== undefined &&
    !displays.some((display) => display.id === settings.selectedDisplay)
  const displayError =
    state.displayError ??
    (selectedDisplayMissing
      ? 'The selected display is unavailable; choose Automatic or another detected display.'
      : null)
  const displayLoadState = displayError
    ? 'error'
    : state.displayStatus === 'idle'
      ? 'loading'
      : state.displayStatus

  const integrationCards: IntegrationCardState[] = state.adapters.map((adapter) => {
    const healthEntry = health?.adapters.find((entry) => entry.source === adapter.source)
    const lastEventAtMs = sessions
      .filter((session) => session.source === adapter.source)
      .reduce<number | undefined>(
        (latest, session) =>
          latest === undefined ? session.lastEventAtMs : Math.max(latest, session.lastEventAtMs),
        undefined,
      )
    return {
      adapter,
      configured: lastEventAtMs !== undefined,
      lastEventAtMs,
      health: mapConnectorStatusToDashboardHealth(healthEntry?.status),
      previewConfig: `Read-only template: ${templatePath(adapter.source)}`,
    }
  })

  const updateSettings = (patch: Partial<PublicSettings>) => {
    setActionError(undefined)
    void client.updateSettings({ ...settings, ...patch }).catch((error: unknown) => {
      setActionError(error instanceof Error ? error.message : 'Settings update failed')
    })
  }

  const updateDisplay = (displayId: string | null) => {
    if (displayId) {
      updateSettings({ selectedDisplay: displayId })
      return
    }
    const nextSettings = { ...settings }
    delete nextSettings.selectedDisplay
    void client.updateSettings(nextSettings).catch((error: unknown) => {
      setActionError(error instanceof Error ? error.message : 'Display update failed')
    })
  }

  const closeOnboarding = () => {
    saveOnboardingComplete()
    setOnboardingOpen(false)
  }

  const previewIntegration = (source: AgentSource) => {
    setActionError(undefined)
    void client
      .previewConnector(source)
      .then((preview) => {
        setPendingDiff({
          source,
          summary: preview.summary,
          before: 'No files inspected or changed by llm_notch.',
          after: `Review manually: ${templatePath(source)}`,
        })
      })
      .catch((error: unknown) => {
        setActionError(error instanceof Error ? error.message : 'Connector preview failed')
      })
  }

  const historyRange = state.historyRange as MetricsHistoryRange
  const disabledHistoryRanges: MetricsHistoryRange[] = [
    ...(settings.historyRetentionHours < 1 ? (['1h'] as const) : []),
    ...(settings.historyRetentionHours < 24 ? (['24h'] as const) : []),
  ]
  const selectedHistory =
    historyRange === '15m'
      ? filterHistory(liveHistory, historyRange)
      : state.history
        ? persistedHistoryBundle(state.history, sessions)
        : emptyHistory(
            Date.now() - (historyRange === '1h' ? 60 * 60_000 : 24 * 60 * 60_000),
            Date.now(),
          )
  const liveHistoryEmpty =
    selectedHistory.hostCpu.length === 0 &&
    selectedHistory.aggregateCpu.length === 0 &&
    selectedHistory.perAgent.every((series) => series.cpu.length === 0)
  const historyLoadState =
    historyRange === '15m'
      ? liveHistoryEmpty
        ? 'empty'
        : 'ready'
      : state.historyStatus === 'idle'
        ? 'loading'
        : state.historyStatus

  useEffect(() => {
    if (
      (historyRange === '24h' && settings.historyRetentionHours < 24) ||
      (historyRange === '1h' && settings.historyRetentionHours < 1)
    ) {
      dispatch({ type: 'SET_HISTORY_RANGE', range: '15m' })
    }
  }, [dispatch, historyRange, settings.historyRetentionHours])

  return (
    <>
      <div
        data-dashboard-background
        inert={onboardingOpen ? true : undefined}
        aria-hidden={onboardingOpen ? 'true' : undefined}
      >
        {actionError ? (
          <p role="alert" style={{ padding: '0.5rem 1rem', color: 'var(--color-error)' }}>
            {actionError}
          </p>
        ) : null}
        <DashboardShell
          loadState={loadState}
          errorMessage={state.errorMessage ?? undefined}
          activeTab={activeTab}
          onTabChange={setActiveTab}
          shortcutsEnabled={!onboardingOpen}
          reducedMotion={prefersReducedMotion}
          sessionsPanel={
            <SessionsPanel
              sessions={sessions}
              selectedSessionId={state.selectedSessionId ?? undefined}
              events={state.events}
              adapters={state.adapters}
              onSelectSession={(sessionId) => dispatch({ type: 'SET_SELECTED_SESSION', sessionId })}
              onOpenContext={(sessionId) => {
                void client.openSession(sessionId).catch((error: unknown) => {
                  setActionError(error instanceof Error ? error.message : 'Context open failed')
                })
              }}
              loadState={sessions.length === 0 && loadState === 'ready' ? 'empty' : loadState}
            />
          }
          metricsPanel={
            <MetricsPanel
              host={state.metrics?.host ?? snapshot?.host}
              aggregate={state.metrics?.aggregate ?? snapshot?.aggregate}
              agents={state.metrics?.agents ?? {}}
              history={selectedHistory}
              historyRange={historyRange}
              onHistoryRangeChange={(range) => dispatch({ type: 'SET_HISTORY_RANGE', range })}
              loadState={loadState}
              warmingUp={state.metrics?.aggregate.quality.cpu === 'warmingUp'}
              historyLoadState={historyLoadState}
              historyError={state.historyError ?? undefined}
              disabledHistoryRanges={disabledHistoryRanges}
            />
          }
          integrationsPanel={
            <IntegrationsPanel
              integrations={integrationCards}
              pendingDiff={pendingDiff}
              writeActionsAvailable={false}
              onPreview={previewIntegration}
              onApply={() =>
                setActionError('Automatic connector writes are unavailable in this build.')
              }
              onRemove={() =>
                setActionError('Automatic connector removal is unavailable in this build.')
              }
              onConfirmDiff={() => setPendingDiff(undefined)}
              onCancelDiff={() => setPendingDiff(undefined)}
              loadState={loadState}
            />
          }
          settingsPanel={
            <SettingsPanel
              settings={settings}
              displays={displays}
              displayLoadState={displayLoadState}
              displayError={displayError ?? undefined}
              fullscreenPreferenceSupported={fullscreenPreferenceSupported}
              onDisplayChange={updateDisplay}
              shortcutLabel={SHORTCUT_LABEL}
              onSettingsChange={updateSettings}
              onPurgeHistory={() => setPurgeConfirmOpen(true)}
              purgeConfirmOpen={purgeConfirmOpen}
              onPurgeConfirm={() => {
                void client
                  .purgeHistory()
                  .then(() => {
                    const endMs = Date.now()
                    const emptySeries = {
                      points: [],
                      actualFirstMs: null,
                      actualLastMs: null,
                      totalPoints: 0,
                      returnedPoints: 0,
                      downsampled: false,
                      truncated: false,
                    }
                    setPurgeConfirmOpen(false)
                    setLiveHistory(emptyHistory(endMs - 15 * 60_000, endMs))
                    dispatch({
                      type: 'SET_HISTORY',
                      history: {
                        range: state.historyRange,
                        sinceMs: endMs,
                        endMs,
                        host: emptySeries,
                        aggregate: emptySeries,
                        agents: [],
                      },
                    })
                  })
                  .catch((error: unknown) => {
                    setActionError(error instanceof Error ? error.message : 'History purge failed')
                  })
              }}
              onPurgeCancel={() => setPurgeConfirmOpen(false)}
              loadState={loadState}
            />
          }
        />
      </div>
      <OnboardingFlow
        open={onboardingOpen}
        step={onboardingStep}
        displays={displays}
        selectedDisplayId={settings.selectedDisplay ?? null}
        displayLoadState={displayLoadState}
        displayError={displayError ?? undefined}
        fullscreenPreferenceSupported={fullscreenPreferenceSupported}
        onDisplayChange={updateDisplay}
        integrationOptions={state.adapters
          .map((adapter) => adapter.source)
          .filter((source) => source !== 'unknown')}
        selectedIntegration={onboardingIntegration}
        onIntegrationChange={setOnboardingIntegration}
        shortcutLabel={SHORTCUT_LABEL}
        autostartEnabled={settings.autostartEnabled}
        onAutostartChange={(autostartEnabled) => updateSettings({ autostartEnabled })}
        onNext={() => setOnboardingStep((step) => Math.min(2, step + 1) as OnboardingStep)}
        onBack={() => setOnboardingStep((step) => Math.max(0, step - 1) as OnboardingStep)}
        onSkip={closeOnboarding}
        onFinish={closeOnboarding}
        reducedMotion={prefersReducedMotion}
      />
    </>
  )
}
