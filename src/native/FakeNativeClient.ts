import { NATIVE_COMMANDS } from './commands.ts'
import type { PublicSettings, StreamFrame } from './contracts.ts'
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
  ConnectorPlanPreview,
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

function previewHealthEntry(
  capabilities: ConnectorHealthEntry['capabilities'],
): ConnectorHealthEntry {
  const probes: HealthProbeResult[] = [
    {
      axis: 'installation',
      outcome: 'warn',
      detail: 'Preview template loaded; install state not verified',
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

  return {
    source: capabilities.source,
    status: capabilities.events ? 'waitingFirstEvent' : 'notInstalled',
    probes,
    capabilities,
    detail: capabilities.events
      ? 'Preview adapter ready; awaiting first event'
      : 'Preview adapter limited',
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

  async purgeHistory(): Promise<void> {}

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

  async previewConnector(source: ConnectorPlanPreview['source']): Promise<ConnectorPlanPreview> {
    return {
      planId: `preview-${source}`,
      source,
      scope: 'user',
      summary: 'Preview only — no vendor configuration files were changed.',
      expiresAtMs: Date.now() + 300_000,
      files: [],
      externalTrustActions: [],
    }
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

  /** Test helper to simulate backend resync signals without exposing in production UI. */
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

  /** Test helper to inject an out-of-order frame. */
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

/** Ensures preview clients never masquerade as production hosts. */
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

/** Command map kept for test parity with the Tauri seam. */
export const PREVIEW_COMMAND_SURFACE = NATIVE_COMMANDS
