import { NATIVE_COMMANDS } from './commands.ts'
import type {
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
  StreamFrame,
} from './contracts.ts'
import { PROTOCOL_VERSION } from './contracts.ts'
import { NativeClientError } from './errors.ts'
import {
  createPreviewMetricsFrame,
  createPreviewSnapshot,
  DEFAULT_PUBLIC_SETTINGS,
  PREVIEW_ADAPTERS,
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
  OverlayMode,
  SessionEventPage,
  StreamErrorHandler,
  StreamFrameHandler,
  StreamSubscription,
} from './types.ts'

const METRIC_TICK_MS = 1_000

const PREVIEW_DETECTED: DetectedConnector[] = [
  {
    source: 'cursor',
    scope: 'user',
    displayPath: '~/.cursor/hooks.json',
    configPresent: true,
    managedEntriesPresent: false,
  },
  {
    source: 'claudeCode',
    scope: 'user',
    displayPath: '~/.claude/settings.json',
    configPresent: true,
    managedEntriesPresent: false,
  },
  {
    source: 'codex',
    scope: 'user',
    displayPath: '~/.codex/hooks.json',
    configPresent: false,
    managedEntriesPresent: false,
  },
]

function previewHealthEntry(
  capabilities: ConnectorHealthEntry['capabilities'],
): ConnectorHealthEntry {
  const probes: HealthProbeResult[] = [
    {
      axis: 'installation',
      outcome: capabilities.source === 'codex' ? 'fail' : 'ok',
      ...(capabilities.source === 'codex' ? { failureKind: 'notInstalled' as const } : {}),
    },
    {
      axis: 'trust',
      outcome: capabilities.requiresExternalTrust ? 'warn' : 'ok',
      ...(capabilities.requiresExternalTrust
        ? { failureKind: 'trustRequired' as const }
        : {}),
    },
    {
      axis: 'traffic',
      outcome: capabilities.events ? 'warn' : 'fail',
      ...(capabilities.events ? {} : { failureKind: 'noTraffic' as const }),
    },
    { axis: 'helper', outcome: 'ok' },
  ]

  const status =
    capabilities.source === 'codex'
      ? 'actionNeeded'
      : capabilities.events
        ? 'waitingFirstEvent'
        : 'notInstalled'

  return {
    source: capabilities.source,
    status,
    probes,
    capabilities,
    detail:
      capabilities.source === 'codex'
        ? 'Run /hooks in Codex and trust llm_notch hook definitions.'
        : capabilities.events
          ? 'Preview adapter ready; awaiting first event'
          : 'Preview adapter limited',
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
            displayPath:
              source === 'cursor'
                ? '~/.cursor/hooks.json'
                : source === 'claudeCode'
                  ? '~/.claude/settings.json'
                  : '~/.codex/hooks.json',
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

  async openDashboard(): Promise<void> {}

  async openSession(_sessionId: string): Promise<void> {}

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

  async applyConnectorChange(planId: string): Promise<ConnectorApplyResult> {
    const source = planId.includes('claudeCode')
      ? 'claudeCode'
      : planId.includes('codex')
        ? 'codex'
        : 'cursor'
    const capabilities =
      PREVIEW_ADAPTERS.find((adapter) => adapter.source === source) ?? PREVIEW_ADAPTERS[0]!

    const fileResults: ConnectorFileApplyResult[] = [
      {
        displayPath:
          source === 'cursor'
            ? '~/.cursor/hooks.json'
            : source === 'claudeCode'
              ? '~/.claude/settings.json'
              : '~/.codex/hooks.json',
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
          displayPath:
            source === 'cursor'
              ? '~/.cursor/hooks.json'
              : source === 'claudeCode'
                ? '~/.claude/settings.json'
                : '~/.codex/hooks.json',
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
