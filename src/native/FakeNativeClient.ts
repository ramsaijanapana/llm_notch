import { NATIVE_COMMANDS } from './commands.ts'
import type {
  AgentCatalogEntry,
  AgentSource,
  BackupJournalEntry,
  ConnectorApplyResult,
  ConnectorFileApplyResult,
  ConnectorPlanPreview,
  ConnectorScope,
  DecisionRequest,
  DecisionResponse,
  DecisionResponseRecord,
  DetectedConnector,
  PublicSettings,
  QuotaSnapshotView,
  RemoteBackendStatus,
  RemoteConnectionStatusView,
  RemoteDeploymentPlanView,
  RemoteDeploymentResultView,
  RemoteHostConfigInput,
  RemoteHostView,
  SoundEvent,
  SoundRouting,
  SoundRoutingPreview,
  SoundPlayRequest,
  SoundPlayResult,
  SoundTheme,
  SoundPackValidation,
  ImportSoundPackRequest,
  StreamFrame,
} from './contracts.ts'
import { PROTOCOL_VERSION } from './contracts.ts'
import { NativeClientError } from './errors.ts'
import {
  createPreviewMetricsFrame,
  createPreviewSnapshot,
  DEFAULT_PUBLIC_SETTINGS,
  PREVIEW_ADAPTERS,
  PREVIEW_AGENT_CATALOG,
  PREVIEW_EVENTS,
} from './fixtures.ts'
import { resolveNativePreviewScenario } from './previewRouting.ts'
import { coalesceStreamFrames } from './streamProcessor.ts'
import type {
  BootstrapResult,
  ConnectorHealthEntry,
  ConnectorHealthReport,
  HealthProbeResult,
  NativeClient,
  NativeDisplayOption,
  NativeHistoryRange,
  NativeHistoryResponse,
  OpenSessionResult,
  OverlayMode,
  RemoteConnectionChangeHandler,
  RemoteConnectionSubscription,
  ConnectorHealthChangeHandler,
  ConnectorHealthSubscription,
  SessionEventPage,
  StreamErrorHandler,
  StreamFrameHandler,
  StreamSubscription,
} from './types.ts'

const METRIC_TICK_MS = 1_000

function validateRemoteHostConfigInput(config: RemoteHostConfigInput): void {
  if (
    !config.id ||
    config.id.length > 64 ||
    !/^[a-zA-Z0-9_-]+$/.test(config.id)
  ) {
    throw new NativeClientError('remote-host-invalid', 'remote host id is invalid')
  }
  if (
    !config.destination ||
    config.destination.length > 255 ||
    config.destination.startsWith('-') ||
    /[^a-zA-Z0-9._@%[\]:-]/.test(config.destination)
  ) {
    throw new NativeClientError('remote-host-invalid', 'SSH destination is invalid')
  }
  if (config.port === 0) {
    throw new NativeClientError('remote-host-invalid', 'SSH port must not be zero')
  }
  if (config.connectTimeoutSeconds < 1 || config.connectTimeoutSeconds > 120) {
    throw new NativeClientError(
      'remote-host-invalid',
      'connect timeout must be between 1 and 120 seconds',
    )
  }
}

const PREVIEW_DETECTED: DetectedConnector[] = [
  {
    source: 'cursor',
    scope: 'user',
    displayPath: '~/.cursor/hooks.json',
    configPresent: true,
    managedEntriesPresent: false,
    executablePresent: true,
    executablePath: 'C:\\Program Files\\cursor\\cursor.cmd',
    processRunning: true,
    runningProcessName: 'cursor',
  },
  {
    source: 'claudeCode',
    scope: 'user',
    displayPath: '~/.claude/settings.json',
    configPresent: true,
    managedEntriesPresent: false,
    executablePresent: false,
  },
  {
    source: 'codex',
    scope: 'user',
    displayPath: '~/.codex/hooks.json',
    configPresent: false,
    managedEntriesPresent: false,
    executablePresent: true,
    executablePath: 'C:\\Users\\dev\\AppData\\Roaming\\npm\\codex.cmd',
  },
  {
    source: 'gemini',
    scope: 'user',
    displayPath: '~/.gemini/settings.json',
    configPresent: false,
    managedEntriesPresent: false,
    executablePresent: false,
  },
  {
    source: 'qwen',
    scope: 'user',
    displayPath: '~/.qwen/settings.json',
    configPresent: false,
    managedEntriesPresent: false,
    executablePresent: false,
  },
  {
    source: 'antigravityCli',
    scope: 'user',
    displayPath: '~/.gemini/antigravity-cli/hooks.json',
    configPresent: false,
    managedEntriesPresent: false,
    executablePresent: false,
  },
  {
    source: 'copilotCli',
    scope: 'user',
    displayPath: '~/.copilot/hooks/llm-notch.json',
    configPresent: false,
    managedEntriesPresent: false,
    executablePresent: false,
  },
]

function previewConnectorDisplayPath(source: AgentSource): string {
  switch (source) {
    case 'claudeCode':
      return '~/.claude/settings.json'
    case 'codex':
      return '~/.codex/hooks.json'
    case 'gemini':
      return '~/.gemini/settings.json'
    case 'qwen':
      return '~/.qwen/settings.json'
    case 'antigravityCli':
      return '~/.gemini/antigravity-cli/hooks.json'
    case 'copilotCli':
      return '~/.copilot/hooks/llm-notch.json'
    case 'cursor':
    default:
      return '~/.cursor/hooks.json'
  }
}

function sourceFromPlanId(planId: string): AgentSource {
  for (const adapter of PREVIEW_ADAPTERS) {
    if (planId.includes(adapter.source)) {
      return adapter.source
    }
  }
  return 'cursor'
}

function previewHealthEntry(
  capabilities: ConnectorHealthEntry['capabilities'],
): ConnectorHealthEntry {
  const detected = PREVIEW_DETECTED.find((entry) => entry.source === capabilities.source)
  const installationOutcome = detected?.managedEntriesPresent
    ? ('ok' as const)
    : detected?.configPresent
      ? ('warn' as const)
      : detected?.executablePresent
        ? ('fail' as const)
        : ('fail' as const)
  const installationFailureKind = detected?.managedEntriesPresent
    ? undefined
    : detected?.configPresent
      ? ('configDrift' as const)
      : detected?.executablePresent
        ? ('notInstalled' as const)
        : ('agentNotFound' as const)

  const probes: HealthProbeResult[] = [
    {
      axis: 'installation',
      outcome: installationOutcome,
      ...(installationFailureKind ? { failureKind: installationFailureKind } : {}),
      ...(detected?.executablePresent && !detected.configPresent
        ? {
            detail: detected.executablePath
              ? `CLI installed at ${detected.executablePath} — hook config missing`
              : 'CLI installed — hook config missing',
          }
        : {}),
      ...(detected?.configPresent && !detected.managedEntriesPresent
        ? { detail: 'Hook config present — llm_notch hooks need repair' }
        : {}),
    },
    {
      axis: 'trust',
      outcome: capabilities.requiresExternalTrust ? 'warn' : 'ok',
      ...(capabilities.requiresExternalTrust ? { failureKind: 'trustRequired' as const } : {}),
    },
    {
      axis: 'traffic',
      outcome:
        detected?.managedEntriesPresent && capabilities.events
          ? 'warn'
          : detected?.managedEntriesPresent
            ? 'fail'
            : 'fail',
      ...(!detected?.managedEntriesPresent ? { failureKind: 'noTraffic' as const } : {}),
    },
    { axis: 'helper', outcome: 'ok' },
  ]

  let status: ConnectorHealthEntry['status'] = 'notFound'
  if (detected?.managedEntriesPresent) {
    status = capabilities.events ? 'waitingFirstEvent' : 'waitingFirstEvent'
  } else if (detected?.configPresent) {
    status = 'driftDetected'
  } else if (detected?.executablePresent) {
    status = 'notInstalled'
  } else if (detected) {
    status = detected.configPresent ? 'driftDetected' : 'notFound'
  }

  let detail: string | undefined
  if (detected?.configPresent && !detected.managedEntriesPresent) {
    detail = 'Hook config exists but llm_notch entries are missing — use Repair.'
  } else if (detected?.executablePresent && !detected.configPresent) {
    detail = 'Agent CLI is installed — use Connect to wire llm_notch hooks.'
  } else if (detected?.managedEntriesPresent) {
    detail = 'Hooks installed; waiting for the first agent event.'
  }

  return {
    source: capabilities.source,
    status,
    probes,
    capabilities,
    ...(detail !== undefined ? { detail } : {}),
  }
}

function previewPlan(
  source: AgentSource,
  scope: ConnectorScope,
  operation: 'install' | 'repair' | 'rollback' = 'install',
): ConnectorPlanPreview {
  const files =
    operation === 'rollback'
      ? [
          {
            displayPath: '~/.cursor/hooks.json',
            baselineSha256: 'abc123',
            diffText: '-  "llm_notch": ...\n+  (restored entries)',
            foreignEntriesPreserved: ['other-hook'],
            isNewFile: false,
          },
        ]
      : [
          {
            displayPath: previewConnectorDisplayPath(source),
            baselineSha256: 'abc123',
            diffText:
              operation === 'repair'
                ? '  hooks: {\n+    "llm_notch": { "command": "/path/to/helper" }\n  }'
                : '+  "llm_notch": { "command": "/path/to/helper" }\n   "other-hook": preserved',
            foreignEntriesPreserved: ['other-hook'],
            isNewFile: source === 'codex',
          },
        ]

  return {
    planId: `preview-${source}-${operation}-${scope}`,
    source,
    scope,
    summary:
      operation === 'repair'
        ? `Repair llm_notch entries for ${source}.`
        : operation === 'rollback'
          ? 'Restore this file from backup.'
          : `Connect ${source} using user-scope hooks.`,
    expiresAtMs: Date.now() + 300_000,
    files,
    externalTrustActions:
      source === 'codex'
        ? [
            {
              kind: 'codexHooksReview',
              instructions:
                'Open the Codex CLI, run /hooks, review each llm_notch hook definition, and trust it.',
            },
          ]
        : [],
    backupDisplayHint: '~/.cursor/hooks.json.bak-20260711',
  }
}

export class FakeNativeClient implements NativeClient {
  readonly mode = 'preview' as const

  private sequence = 0
  private subscribed = false
  private metricTimer: ReturnType<typeof setInterval> | null = null
  private frameHandler: StreamFrameHandler | null = null
  private errorHandler: StreamErrorHandler | null = null
  private settings: PublicSettings = { ...DEFAULT_PUBLIC_SETTINGS }
  private overlayMode: OverlayMode = 'collapsed'
  private acknowledgedSessions = new Set<string>()
  private bootstrapCount = 0
  private backups: BackupJournalEntry[] = []
  private pendingDecisions: DecisionRequest[] = [
    {
      id: 'decision-preview-1',
      sessionId: 'sess-claude-review',
      source: 'claudeCode',
      kind: 'approval',
      summary: 'Allow running: npm test --coverage',
      hasActionablePayload: false,
      createdAtMs: Date.now() - 30_000,
      expiresAtMs: Date.now() + 120_000,
    },
  ]
  private decisionRecords = new Map<string, DecisionResponseRecord>()
  private remoteHosts = new Map<string, RemoteHostView>()
  private remoteConnectionHandlers = new Set<RemoteConnectionChangeHandler>()
  private connectorHealthHandlers = new Set<ConnectorHealthChangeHandler>()

  async bootstrap(): Promise<BootstrapResult> {
    const scenario = resolveNativePreviewScenario()

    if (scenario === 'disconnected') {
      throw new NativeClientError('stream-closed', 'Preview host disconnected')
    }

    if (scenario === 'incompatible') {
      throw new NativeClientError(
        'protocol-incompatible',
        'Preview snapshot protocol is incompatible with this renderer build',
      )
    }

    if (scenario === 'loading') {
      await new Promise((resolve) => setTimeout(resolve, 2_500))
    }

    this.bootstrapCount += 1
    if (scenario === 'resync' && this.bootstrapCount > 1) {
      await new Promise((resolve) => setTimeout(resolve, 3_000))
    }

    const snapshot = createPreviewSnapshot()
    snapshot.settings = { ...this.settings }
    this.sequence = 0
    return { snapshot, lastSequence: this.sequence, events: PREVIEW_EVENTS }
  }

  async subscribe(
    onFrame: StreamFrameHandler,
    onError: StreamErrorHandler,
  ): Promise<StreamSubscription> {
    if (this.subscribed) {
      throw new NativeClientError('stream-closed', 'Preview stream already active')
    }

    this.subscribed = true
    this.frameHandler = onFrame
    this.errorHandler = onError

    this.metricTimer = setInterval(() => {
      this.emit({
        sequence: ++this.sequence,
        emittedAtMs: Date.now(),
        payload: { type: 'metrics', metrics: createPreviewMetricsFrame() },
      })
    }, METRIC_TICK_MS)

    return {
      unsubscribe: async () => {
        await this.stopStream()
      },
    }
  }

  async subscribeRemoteConnectionChanges(
    onChange: RemoteConnectionChangeHandler,
  ): Promise<RemoteConnectionSubscription> {
    this.remoteConnectionHandlers.add(onChange)
    return {
      unsubscribe: async () => {
        this.remoteConnectionHandlers.delete(onChange)
      },
    }
  }

  async subscribeConnectorHealthChanges(
    onChange: ConnectorHealthChangeHandler,
  ): Promise<ConnectorHealthSubscription> {
    this.connectorHealthHandlers.add(onChange)
    return {
      unsubscribe: async () => {
        this.connectorHealthHandlers.delete(onChange)
      },
    }
  }

  async openDashboard(): Promise<void> {}

  async openSession(_sessionId: string): Promise<OpenSessionResult> {
    return {
      contextOpenTier: 'appActivate',
      activated: true,
      message: 'Preview context navigation activated.',
    }
  }

  async setOverlayMode(mode: OverlayMode): Promise<void> {
    this.overlayMode = mode
  }

  async acknowledgeLocalAttention(sessionId: string): Promise<void> {
    this.acknowledgedSessions.add(sessionId)
  }

  async updateSettings(settings: PublicSettings): Promise<PublicSettings> {
    this.settings = { ...settings }
    this.emit({
      sequence: ++this.sequence,
      emittedAtMs: Date.now(),
      payload: { type: 'settingsChanged', settings: this.settings },
    })
    return this.settings
  }

  async purgeHistory(_scope?: import('./contracts.ts').PurgeScope): Promise<void> {}

  async getHistory(range: NativeHistoryRange): Promise<NativeHistoryResponse> {
    const endMs = Date.now()
    const sinceMs =
      range === '15m'
        ? endMs - 15 * 60 * 1_000
        : range === '1h'
          ? endMs - 60 * 60 * 1_000
          : endMs - 24 * 60 * 60 * 1_000

    const emptySeries = {
      points: [],
      actualFirstMs: null,
      actualLastMs: null,
      totalPoints: 0,
      returnedPoints: 0,
      downsampled: false,
      truncated: false,
    }

    return {
      range,
      sinceMs,
      endMs,
      host: emptySeries,
      aggregate: emptySeries,
      agents: [],
    }
  }

  async getSessionEvents(
    sessionId: string,
    beforeSequence?: number,
    limit = 50,
  ): Promise<SessionEventPage> {
    const eligible = PREVIEW_EVENTS.filter((event) => event.sessionId === sessionId)
      .filter((event) => beforeSequence === undefined || event.sequence < beforeSequence)
      .sort((left, right) => right.sequence - left.sequence)
    const page = eligible.slice(0, limit).reverse()

    return {
      sessionId,
      events: page,
      ...(eligible.length > limit && page[0] ? { nextBeforeSequence: page[0].sequence } : {}),
      hasMore: eligible.length > limit,
    }
  }

  async listDisplays(): Promise<NativeDisplayOption[]> {
    return [{ id: 'preview-primary', label: 'Preview Display', primary: true }]
  }

  async getIntegrationHealth(): Promise<ConnectorHealthReport> {
    return {
      checkedAtMs: Date.now(),
      adapters: PREVIEW_ADAPTERS.map((capabilities) => previewHealthEntry(capabilities)),
    }
  }

  async listAgentCatalog(): Promise<AgentCatalogEntry[]> {
    return PREVIEW_AGENT_CATALOG.map((entry) => ({
      ...entry,
      aliases: [...entry.aliases],
      executableNames: [...entry.executableNames],
      capabilities: entry.capabilities.map((capability) => ({ ...capability })),
      configTargets: entry.configTargets.map((target) => ({ ...target })),
    }))
  }

  async listQuotaSnapshots(): Promise<QuotaSnapshotView[]> {
    return ['Claude', 'Codex', 'Gemini', 'Kimi', 'GLM', 'DeepSeek'].map((displayName) => ({
      service: displayName.toLowerCase(),
      displayName,
      availability: 'unavailable',
      message: 'quota provider is not configured',
    }))
  }

  async listRemoteHosts(): Promise<RemoteHostView[]> {
    return [...this.remoteHosts.values()].sort((left, right) =>
      left.config.id.localeCompare(right.config.id),
    )
  }

  async upsertRemoteHost(config: RemoteHostConfigInput): Promise<RemoteHostView> {
    validateRemoteHostConfigInput(config)
    const view: RemoteHostView = {
      config: {
        id: config.id,
        destination: config.destination,
        port: config.port ?? null,
        identityFile: config.identityFile ?? null,
        hostKeyPolicy: config.hostKeyPolicy,
        connectTimeoutSeconds: config.connectTimeoutSeconds,
      },
      availability: 'unavailable',
      connectionState: 'disconnected',
      message: null,
      lastConnectedAtMs: null,
    }
    this.remoteHosts.set(config.id, view)
    return view
  }

  async removeRemoteHost(hostId: string): Promise<void> {
    if (!this.remoteHosts.delete(hostId)) {
      throw new NativeClientError(
        'remote-host-missing',
        `remote host \`${hostId}\` is not configured`,
      )
    }
  }

  async getRemoteBackendStatus(): Promise<RemoteBackendStatus> {
    return {
      availability: 'unavailable',
      message: 'SSH relay backend is not available in this build. Relay lifecycle is owned by notch-remote.',
    }
  }

  async previewRemoteDeploy(hostId: string): Promise<RemoteDeploymentPlanView> {
    throw new NativeClientError(
      'remote-backend-unavailable',
      `SSH relay backend is not available in this build (host: ${hostId})`,
    )
  }

  async executeRemoteDeploy(hostId: string): Promise<RemoteDeploymentResultView> {
    throw new NativeClientError(
      'remote-backend-unavailable',
      `SSH relay backend is not available in this build (host: ${hostId})`,
    )
  }

  async startRemoteRelay(hostId: string): Promise<RemoteConnectionStatusView> {
    throw new NativeClientError(
      'remote-backend-unavailable',
      `SSH relay backend is not available in this build (host: ${hostId})`,
    )
  }

  async stopRemoteRelay(hostId: string): Promise<RemoteConnectionStatusView> {
    throw new NativeClientError(
      'remote-backend-unavailable',
      `SSH relay backend is not available in this build (host: ${hostId})`,
    )
  }

  async getRemoteConnectionStatus(hostId: string): Promise<RemoteConnectionStatusView> {
    return {
      hostId,
      availability: 'unavailable',
      connectionState: 'disconnected',
      message: 'SSH relay backend is not available in this build. Relay lifecycle is owned by notch-remote.',
    }
  }

  async getSoundThemes(): Promise<SoundTheme[]> {
    return [
      {
        schemaVersion: 1,
        id: 'builtin.8-bit',
        name: '8-Bit Signals',
        author: 'LLM Notch',
        events: {},
      },
    ]
  }

  async previewSoundRouting(request: {
    routing: SoundRouting
    event: SoundEvent
    agent?: string
    localMinute: number
  }): Promise<SoundRoutingPreview> {
    const audible = request.routing.enabled && request.routing.volume > 0
    return {
      audible,
      ...(audible ? { effectiveVolume: request.routing.volume } : {}),
      ...(audible ? {} : { reason: 'sound is disabled' }),
    }
  }

  async playSoundEvent(request: SoundPlayRequest): Promise<SoundPlayResult> {
    const audible = request.routing.enabled && request.routing.volume > 0
    return {
      played: audible,
      backendId: 'preview',
      ...(audible ? { effectiveVolume: request.routing.volume } : {}),
      ...(audible ? {} : { reason: 'sound is disabled' }),
    }
  }

  async importSoundPack(_request: ImportSoundPackRequest): Promise<SoundPackValidation> {
    throw new NativeClientError('invoke-failed', 'Sound pack import is unavailable in preview mode')
  }

  async detectConnectors(): Promise<DetectedConnector[]> {
    await new Promise((resolve) => setTimeout(resolve, 400))
    return PREVIEW_DETECTED.map((entry) => ({ ...entry }))
  }

  async previewConnector(
    source: ConnectorPlanPreview['source'],
    scope: ConnectorScope = 'user',
  ): Promise<ConnectorPlanPreview> {
    return previewPlan(source, scope, 'install')
  }

  async applyConnectorChange(
    planId: string,
    _selectedDisplayPaths?: string[],
  ): Promise<ConnectorApplyResult> {
    const source = sourceFromPlanId(planId)
    const capabilities =
      PREVIEW_ADAPTERS.find((adapter) => adapter.source === source) ?? PREVIEW_ADAPTERS[0]!

    const fileResults: ConnectorFileApplyResult[] = [
      {
        displayPath: previewConnectorDisplayPath(source),
        outcome: 'applied',
        backupJournalId: `backup-${source}-1`,
        appliedHash: 'hash-applied-1',
      },
    ]

    if (planId.includes('partial')) {
      fileResults.push({
        displayPath: '~/.cursor/hooks.secondary.json',
        outcome: 'failed',
        errorCode: 'lockContention',
        message: 'File locked by another process',
      })
    }

    for (const result of fileResults) {
      if (result.backupJournalId) {
        this.backups.push({
          id: result.backupJournalId,
          planId,
          source,
          displayPath: result.displayPath,
          backupDisplayPath: `${result.displayPath}.bak-preview`,
          contentSha256: 'sha-backup',
          ...(result.appliedHash ? { appliedHash: result.appliedHash } : {}),
          operation: 'create',
          recordedAtMs: Date.now(),
        })
      }
    }

    return { planId, source, fileResults, capabilities }
  }

  async removeConnector(
    source: AgentSource,
    _scope: ConnectorScope = 'user',
  ): Promise<ConnectorApplyResult> {
    const capabilities =
      PREVIEW_ADAPTERS.find((adapter) => adapter.source === source) ?? PREVIEW_ADAPTERS[0]!
    return {
      planId: `remove-${source}`,
      source,
      fileResults: [
        {
          displayPath: previewConnectorDisplayPath(source),
          outcome: 'applied',
        },
      ],
      capabilities,
    }
  }

  async repairConnector(
    source: AgentSource,
    scope: ConnectorScope = 'user',
  ): Promise<ConnectorPlanPreview> {
    return previewPlan(source, scope, 'repair')
  }

  async rollbackConnector(backupId: string): Promise<ConnectorPlanPreview> {
    const backup = this.backups.find((entry) => entry.id === backupId)
    return previewPlan(backup?.source ?? 'cursor', 'user', 'rollback')
  }

  async listConnectorBackups(): Promise<BackupJournalEntry[]> {
    return [...this.backups]
  }

  async getPendingDecisions(): Promise<DecisionRequest[]> {
    return this.pendingDecisions.filter((request) => !this.decisionRecords.has(request.id))
  }

  async respondDecision(
    requestId: string,
    response: DecisionResponse,
  ): Promise<DecisionResponseRecord> {
    const request = this.pendingDecisions.find((entry) => entry.id === requestId)
    if (!request) {
      throw new NativeClientError('not-available', `Unknown decision request: ${requestId}`)
    }
    const record: DecisionResponseRecord = {
      requestId,
      response,
      respondedAtMs: Date.now(),
      deliveryState: request.hasActionablePayload ? 'pending' : 'delivered',
      ...(request.hasActionablePayload
        ? { deliveryDetail: 'Awaiting agent hook delivery confirmation.' }
        : {}),
    }
    this.decisionRecords.set(requestId, record)
    return record
  }

  getOverlayModeForTests(): OverlayMode {
    return this.overlayMode
  }

  wasAttentionAcknowledged(sessionId: string): boolean {
    return this.acknowledgedSessions.has(sessionId)
  }

  private emit(frame: StreamFrame): void {
    if (!this.frameHandler) {
      return
    }

    const [coalesced] = coalesceStreamFrames([frame])
    if (coalesced) {
      this.frameHandler(coalesced)
    }
  }

  private async stopStream(): Promise<void> {
    if (this.metricTimer) {
      clearInterval(this.metricTimer)
      this.metricTimer = null
    }

    this.subscribed = false
    this.frameHandler = null
    this.errorHandler = null
  }

  simulateResyncRequired(reason: string): void {
    this.errorHandler?.(
      new NativeClientError('resync-required', reason || 'Preview stream requested resync'),
    )
    this.emit({
      sequence: ++this.sequence,
      emittedAtMs: Date.now(),
      payload: { type: 'resyncRequired', reason },
    })
  }

  simulateSequenceGap(): void {
    this.sequence += 5
    this.emit({
      sequence: this.sequence,
      emittedAtMs: Date.now(),
      payload: { type: 'heartbeat' },
    })
  }

  simulateRemoteConnectionChange(status: RemoteConnectionStatusView): void {
    const host = this.remoteHosts.get(status.hostId)
    if (host) {
      this.remoteHosts.set(status.hostId, {
        ...host,
        availability: status.availability,
        connectionState: status.connectionState,
        message: status.message ?? null,
      })
    }

    for (const handler of this.remoteConnectionHandlers) {
      handler(status)
    }
  }
}

export function createFakeNativeClient(): FakeNativeClient {
  return new FakeNativeClient()
}

export function createPreviewNativeClient(): FakeNativeClient {
  return new FakeNativeClient()
}

export function isPreviewNativeClient(client: NativeClient): client is FakeNativeClient {
  return client.mode === 'preview'
}

export function assertPreviewNativeClient(client: NativeClient): FakeNativeClient {
  if (!isPreviewNativeClient(client)) {
    throw new NativeClientError('not-available', 'Expected preview native client')
  }
  return client
}

export function assertPreviewClient(client: NativeClient): asserts client is FakeNativeClient {
  assertPreviewNativeClient(client)
}

export function validatePreviewProtocol(snapshotProtocolVersion: number): void {
  if (snapshotProtocolVersion !== PROTOCOL_VERSION) {
    throw new NativeClientError(
      'protocol-incompatible',
      `Preview snapshot protocol ${snapshotProtocolVersion} does not match renderer ${PROTOCOL_VERSION}`,
    )
  }
}

export const PREVIEW_COMMAND_SURFACE = NATIVE_COMMANDS
