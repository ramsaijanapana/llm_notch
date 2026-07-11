import { useEffect, useMemo, useState } from 'react'
import {
  agentLabel,
  type ApplyProgressEntry,
  type ConnectFileSelection,
  type DashboardLoadState,
  DashboardShell,
  type DashboardTab,
  type IntegrationCardState,
  type MetricSeriesCoverage,
  type MetricsHistoryBundle,
  type MetricsHistoryRange,
  IntegrationsPanel,
  MetricsPanel,
  type OnboardingStep,
  type PendingPlanReview,
  SessionsPanel,
  SettingsPanel,
  OnboardingFlow,
} from '../features/native-dashboard'
import {
  type OverlayConnectionState,
  type OverlayCpuSample,
  type OverlayMode,
  type OverlayPlatform,
  OverlayShell,
} from '../features/native-overlay'
import type {
  AgentSession,
  AgentSource,
  AppSnapshot,
  ConnectorApplyResult,
  ConnectorFileApplyResult,
  ConnectorScope,
  DecisionRequest,
  DecisionResponseRecord,
  DetectedConnector,
  PublicSettings,
} from '../native/contracts.ts'
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

async function runApplyProgress(
  planId: string,
  filePaths: string[],
  apply: (planId: string, selectedDisplayPaths?: string[]) => Promise<ConnectorApplyResult>,
  onProgress: (entries: ApplyProgressEntry[]) => void,
  selectedDisplayPaths?: string[],
): Promise<ConnectorApplyResult> {
  onProgress(
    filePaths.map((displayPath) => ({
      displayPath,
      phase: 'applying' as const,
    })),
  )

  const result = await apply(planId, selectedDisplayPaths)
  onProgress(
    result.fileResults.map((file) => ({
      displayPath: file.displayPath,
      phase: mapFileResultPhase(file.outcome),
      message:
        file.outcome === 'failed'
          ? (file.message ?? 'Apply failed for this file')
          : file.outcome === 'skipped'
            ? (file.message ?? 'No changes needed')
            : undefined,
    })),
  )
  return result
}

function mapFileResultPhase(
  outcome: ConnectorFileApplyResult['outcome'],
): ApplyProgressEntry['phase'] {
  return outcome === 'failed' ? 'failed' : 'done'
}

function defaultConnectorStatus(): ConnectorUserStatus {
  return 'notInstalled'
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
  const [backups, setBackups] = useState<import('../native/contracts.ts').BackupJournalEntry[]>([])
  const [pendingPlan, setPendingPlan] = useState<PendingPlanReview>()
  const [pendingPlanQueue, setPendingPlanQueue] = useState<PendingPlanReview[]>([])
  const [applyProgress, setApplyProgress] = useState<ApplyProgressEntry[]>()
  const [applyResult, setApplyResult] = useState<ConnectorApplyResult>()
  const [actionError, setActionError] = useState<string>()
  const [purgeConfirmOpen, setPurgeConfirmOpen] = useState(false)
  const [purgeScope, setPurgeScope] = useState<import('../native/contracts.ts').PurgeScope>({
    history: true,
    sessionEvents: true,
    connectorJournal: false,
    includeBackups: false,
  })
  const [onboardingOpen, setOnboardingOpen] = useState(() => !readOnboardingComplete())
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep>(0)
  const [detectedConnectors, setDetectedConnectors] = useState<DetectedConnector[]>([])
  const [detectLoadState, setDetectLoadState] = useState<'idle' | 'loading' | 'ready' | 'error'>(
    'idle',
  )
  const [detectError, setDetectError] = useState<string>()
  const [connectSelections, setConnectSelections] = useState<ConnectFileSelection[]>([])
  const [connectScope, setConnectScope] = useState<ConnectorScope>('user')
  const [pendingDecisions, setPendingDecisions] = useState<DecisionRequest[]>([])
  const [decisionRecords, setDecisionRecords] = useState<
    Record<string, DecisionResponseRecord>
  >({})
  const writeActionsAvailable = client.mode === 'preview' || client.mode === 'tauri'

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
        requestedStartMs: Math.max(current.requestedStartMs, Date.now() - 15 * 60_000),
        requestedEndMs: Date.now(),
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
    const refreshPendingDecisions = () => {
      void client
        .getPendingDecisions()
        .then((requests) => {
          if (!cancelled) setPendingDecisions(requests)
        })
        .catch(() => {
          if (!cancelled) setPendingDecisions([])
        })
    }
    refreshPendingDecisions()
    const interval = window.setInterval(refreshPendingDecisions, 5_000)
    const onFocus = () => refreshPendingDecisions()
    window.addEventListener('focus', onFocus)
    return () => {
      cancelled = true
      window.clearInterval(interval)
      window.removeEventListener('focus', onFocus)
    }
  }, [client])

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
    void client
      .listConnectorBackups()
      .then((entries) => {
        if (!cancelled) setBackups(entries)
      })
      .catch(() => {
        if (!cancelled) setBackups([])
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

  const integrationCards: IntegrationCardState[] = state.adapters
    .filter((adapter) => adapter.source !== 'unknown' && adapter.source !== 'generic')
    .map((adapter) => {
      const healthEntry = health?.adapters.find((entry) => entry.source === adapter.source)
      const detected = detectedConnectors.find((entry) => entry.source === adapter.source)
      const lastEventAtMs = sessions
        .filter((session) => session.source === adapter.source)
        .reduce<number | undefined>(
          (latest, session) =>
            latest === undefined ? session.lastEventAtMs : Math.max(latest, session.lastEventAtMs),
          undefined,
        )
      return {
        adapter,
        status: healthEntry?.status ?? defaultConnectorStatus(),
        statusDetail: healthEntry?.detail,
        lastEventAtMs,
        managedEntriesPresent:
          detected?.managedEntriesPresent ?? healthEntry?.status === 'connected',
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

  const refreshBackups = () => {
    void client
      .listConnectorBackups()
      .then(setBackups)
      .catch(() => setBackups([]))
  }

  const refreshHealth = () => {
    void client
      .getIntegrationHealth()
      .then(setHealth)
      .catch((error: unknown) => {
        setActionError(error instanceof Error ? error.message : 'Health check failed')
      })
  }

  const runDetection = () => {
    setDetectLoadState('loading')
    setDetectError(undefined)
    void client
      .detectConnectors()
      .then((detected) => {
        setDetectedConnectors(detected)
        setConnectSelections(
          detected.map((entry) => ({
            source: entry.source,
            displayPath: entry.displayPath,
            selected: entry.configPresent,
          })),
        )
        setDetectLoadState('ready')
      })
      .catch((error: unknown) => {
        setDetectLoadState('error')
        setDetectError(error instanceof Error ? error.message : 'Detection failed')
      })
  }

  const togglePlanFile = (displayPath: string, selected: boolean) => {
    setPendingPlan((current) => {
      if (!current) return current
      const selectedFilePaths = selected
        ? [...new Set([...current.selectedFilePaths, displayPath])]
        : current.selectedFilePaths.filter((path) => path !== displayPath)
      const next = { ...current, selectedFilePaths }
      setPendingPlanQueue((queue) =>
        queue.map((entry) => (entry.plan.planId === current.plan.planId ? next : entry)),
      )
      return next
    })
    setConnectSelections((current) =>
      current.map((entry) =>
        entry.displayPath === displayPath ? { ...entry, selected } : entry,
      ),
    )
  }

  const previewConnect = (source: AgentSource, scope: ConnectorScope = 'user') => {
    setActionError(undefined)
    void client
      .previewConnector(source, scope)
      .then((plan) => {
        const review = {
          plan,
          selectedFilePaths: plan.files.map((file) => file.displayPath),
        }
        setPendingPlan(review)
        setPendingPlanQueue([review])
      })
      .catch((error: unknown) => {
        setActionError(error instanceof Error ? error.message : 'Connector preview failed')
      })
  }

  const applyPendingPlan = () => {
    const plansToApply =
      pendingPlanQueue.length > 0
        ? pendingPlanQueue
        : pendingPlan
          ? [pendingPlan]
          : []
    if (plansToApply.length === 0) return
    setActionError(undefined)
    const allFilePaths = plansToApply.flatMap((entry) => entry.selectedFilePaths)
    setApplyProgress(
      allFilePaths.map((displayPath) => ({ displayPath, phase: 'applying' as const })),
    )
    void (async () => {
      const aggregatedResults: ConnectorApplyResult['fileResults'] = []
      let lastResult: ConnectorApplyResult | undefined
      try {
        for (const entry of plansToApply) {
          lastResult = await runApplyProgress(
            entry.plan.planId,
            entry.selectedFilePaths,
            (planId, selectedDisplayPaths) =>
              client.applyConnectorChange(planId, selectedDisplayPaths),
            setApplyProgress,
            entry.selectedFilePaths,
          )
          aggregatedResults.push(...lastResult.fileResults)
        }
        if (lastResult) {
          setApplyResult({ ...lastResult, fileResults: aggregatedResults })
        }
        setPendingPlan(undefined)
        setPendingPlanQueue([])
        refreshBackups()
        refreshHealth()
      } catch (error: unknown) {
        setApplyProgress(
          allFilePaths.map((displayPath) => ({
            displayPath,
            phase: 'failed',
            message: error instanceof Error ? error.message : 'Apply failed',
          })),
        )
        setActionError(error instanceof Error ? error.message : 'Connector apply failed')
      }
    })()
  }

  const previewAllSelectedConnectors = () => {
    const selectedSources = [
      ...new Set(
        connectSelections.filter((entry) => entry.selected).map((entry) => entry.source),
      ),
    ]
    if (selectedSources.length === 0) return
    setActionError(undefined)
    void (async () => {
      try {
        const reviews: PendingPlanReview[] = []
        for (const source of selectedSources) {
          const plan = await client.previewConnector(source, connectScope)
          reviews.push({
            plan,
            selectedFilePaths: plan.files
              .filter((file) => {
                const selection = connectSelections.find(
                  (entry) => entry.source === source && entry.displayPath === file.displayPath,
                )
                return selection?.selected ?? true
              })
              .map((file) => file.displayPath),
          })
        }
        setPendingPlanQueue(reviews)
        setPendingPlan(reviews[0])
        setOnboardingStep(3)
      } catch (error: unknown) {
        setActionError(error instanceof Error ? error.message : 'Connector preview failed')
      }
    })()
  }

  const respondToDecision = (
    requestId: string,
    response: import('../native/contracts.ts').DecisionResponse,
  ) => {
    void client
      .respondDecision(requestId, response)
      .then((record) => {
        setDecisionRecords((current) => ({ ...current, [requestId]: record }))
      })
      .catch((error: unknown) => {
        setActionError(error instanceof Error ? error.message : 'Decision response failed')
      })
  }

  const selectedDecision = pendingDecisions[0]
  const selectedDecisionRecord = selectedDecision
    ? decisionRecords[selectedDecision.id]
    : undefined

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
              pendingDecision={selectedDecision}
              decisionRecord={selectedDecisionRecord}
              onSelectSession={(sessionId) => dispatch({ type: 'SET_SELECTED_SESSION', sessionId })}
              onOpenContext={(sessionId) => {
                void client.openSession(sessionId).catch((error: unknown) => {
                  setActionError(error instanceof Error ? error.message : 'Context open failed')
                })
              }}
              onDecisionAllow={() =>
                selectedDecision
                  ? respondToDecision(selectedDecision.id, { type: 'action', action: 'allow' })
                  : undefined
              }
              onDecisionDeny={() =>
                selectedDecision
                  ? respondToDecision(selectedDecision.id, { type: 'action', action: 'deny' })
                  : undefined
              }
              onDecisionAnswer={(text) =>
                selectedDecision
                  ? respondToDecision(selectedDecision.id, { type: 'answer', text })
                  : undefined
              }
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
              backups={backups}
              pendingPlan={pendingPlan}
              applyProgress={applyProgress}
              applyResult={applyResult}
              writeActionsAvailable={writeActionsAvailable}
              onConnect={(source) => previewConnect(source, 'user')}
              onRepair={(source) => {
                void client
                  .repairConnector(source, 'user')
                  .then((plan) =>
                    setPendingPlan({
                      plan,
                      selectedFilePaths: plan.files.map((file) => file.displayPath),
                    }),
                  )
                  .catch((error: unknown) => {
                    setActionError(
                      error instanceof Error ? error.message : 'Connector repair preview failed',
                    )
                  })
              }}
              onDisable={(source) => {
                void client
                  .removeConnector(source, 'user')
                  .then(() => refreshHealth())
                  .catch((error: unknown) => {
                    setActionError(
                      error instanceof Error ? error.message : 'Connector disable failed',
                    )
                  })
              }}
              onConfirmPlan={applyPendingPlan}
              onCancelPlan={() => {
                setPendingPlan(undefined)
                setPendingPlanQueue([])
              }}
              onTogglePlanFile={togglePlanFile}
              onRestoreBackup={(backupId) => {
                void client
                  .rollbackConnector(backupId)
                  .then((plan) =>
                    setPendingPlan({
                      plan,
                      selectedFilePaths: plan.files.map((file) => file.displayPath),
                    }),
                  )
                  .catch((error: unknown) => {
                    setActionError(
                      error instanceof Error ? error.message : 'Rollback preview failed',
                    )
                  })
              }}
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
              purgeScope={purgeScope}
              onPurgeScopeChange={(patch) => setPurgeScope((current) => ({ ...current, ...patch }))}
              purgeConfirmOpen={purgeConfirmOpen}
              onPurgeConfirm={() => {
                void client
                  .purgeHistory(purgeScope)
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
          .filter((source) => source !== 'unknown' && source !== 'generic')}
        detectedConnectors={detectedConnectors}
        detectLoadState={detectLoadState}
        detectError={detectError}
        onGetStarted={runDetection}
        connectSelections={connectSelections}
        onConnectSelectionChange={setConnectSelections}
        connectScope={connectScope}
        onConnectScopeChange={setConnectScope}
        pendingPlan={onboardingStep === 3 ? pendingPlan : undefined}
        pendingPlanCount={pendingPlanQueue.length || (pendingPlan ? 1 : 0)}
        applyProgress={onboardingStep === 3 ? applyProgress : undefined}
        applyResult={onboardingStep === 3 ? applyResult : undefined}
        onPreviewConnect={previewAllSelectedConnectors}
        onTogglePlanFile={togglePlanFile}
        onConfirmApply={() => {
          applyPendingPlan()
        }}
        onSkipConnect={() => {
          setPendingPlan(undefined)
          setPendingPlanQueue([])
          setOnboardingStep(4)
        }}
        shortcutLabel={SHORTCUT_LABEL}
        autostartEnabled={settings.autostartEnabled}
        onAutostartChange={(autostartEnabled) => updateSettings({ autostartEnabled })}
        onNext={() => setOnboardingStep((step) => Math.min(4, step + 1) as OnboardingStep)}
        onBack={() => setOnboardingStep((step) => Math.max(0, step - 1) as OnboardingStep)}
        onSkip={closeOnboarding}
        onFinish={closeOnboarding}
        reducedMotion={prefersReducedMotion}
      />
    </>
  )
}
