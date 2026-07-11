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
  ConnectorPreview,
  IntegrationHealthEntry,
  IntegrationHealthReport,
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

function integrationStatusForAdapter(
  adapter: IntegrationHealthEntry['capabilities'],
): IntegrationHealthEntry['status'] {
  if (!adapter.events) {
    return 'unavailable'
  }

  if (adapter.attention === 'none' || adapter.processAttribution === 'unknown') {
    return 'degraded'
  }

  return 'healthy'
}

function summarizeIntegrationHealth(
  adapters: IntegrationHealthEntry[],
): IntegrationHealthReport['overall'] {
  if (adapters.every((entry) => entry.status === 'healthy')) {
    return 'healthy'
  }

  if (adapters.some((entry) => entry.status === 'healthy' || entry.status === 'degraded')) {
    return 'degraded'
  }

  return 'unavailable'
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
      throw new NativeClientError('not-available', 'Preview stream is already active')
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

    if (resolveNativePreviewScenario() === 'resync') {
      queueMicrotask(() => {
        this.simulateResyncRequired('Preview stream requested resync')
      })
    }

    return {
      unsubscribe: async () => {
        await this.stopStream()
      },
    }
  }

  async openDashboard(): Promise<void> {
    return
  }

  async openSession(_sessionId: string): Promise<void> {
    return
  }

  async setOverlayMode(mode: OverlayMode): Promise<void> {
    this.overlayMode = mode
  }

  async acknowledgeLocalAttention(sessionId: string): Promise<void> {
    this.acknowledgedSessions.add(sessionId)
    const session = createPreviewSnapshot().sessions.find((entry) => entry.id === sessionId)
    if (!session) {
      return
    }

    this.emit({
      sequence: ++this.sequence,
      emittedAtMs: Date.now(),
      payload: {
        type: 'sessionUpsert',
        session: { ...session, attention: 'none', status: 'running' },
      },
    })
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

  async purgeHistory(): Promise<void> {
    return
  }

  async getHistory(range: NativeHistoryRange): Promise<NativeHistoryResponse> {
    const metrics = createPreviewMetricsFrame()
    const duration = range === '15m' ? 15 * 60_000 : range === '1h' ? 60 * 60_000 : 24 * 60 * 60_000
    const endMs = Date.now()
    const series = (points: NativeHistoryResponse['host']['points']) => ({
      points,
      actualFirstMs: points[0]?.atMs ?? null,
      actualLastMs: points.at(-1)?.atMs ?? null,
      totalPoints: points.length,
      returnedPoints: points.length,
      downsampled: false,
      truncated: false,
    })
    return {
      range,
      sinceMs: endMs - duration,
      endMs,
      host: series([
        {
          atMs: metrics.host.atMs,
          cpuHostPercent: metrics.host.cpuHostPercent,
          cpuCorePercent: metrics.host.cpuHostPercent,
          rssBytes: metrics.host.usedMemoryBytes,
        },
      ]),
      aggregate: series([
        {
          atMs: metrics.aggregate.atMs,
          cpuHostPercent: metrics.aggregate.cpuHostPercent,
          cpuCorePercent: metrics.aggregate.cpuCorePercent,
          rssBytes: metrics.aggregate.rssBytes,
        },
      ]),
      agents: Object.entries(metrics.agents).map(([sessionId, sample]) => ({
        sessionId,
        ...series([
          {
            atMs: sample.atMs,
            cpuHostPercent: sample.cpuHostPercent,
            cpuCorePercent: sample.cpuCorePercent,
            rssBytes: sample.rssBytes,
          },
        ]),
      })),
    }
  }

  async listDisplays(): Promise<NativeDisplayOption[]> {
    return [{ id: 'preview-primary', label: 'Preview display', primary: true }]
  }

  async getSessionEvents(
    sessionId: string,
    beforeSequence?: number,
    limit = 50,
  ): Promise<SessionEventPage> {
    const eligible = PREVIEW_EVENTS.filter(
      (event) =>
        event.sessionId === sessionId &&
        (beforeSequence === undefined || event.sequence < beforeSequence),
    ).sort((left, right) => right.sequence - left.sequence)
    const page = eligible.slice(0, limit).reverse()
    return {
      sessionId,
      events: page,
      ...(eligible.length > limit && page[0] ? { nextBeforeSequence: page[0].sequence } : {}),
      hasMore: eligible.length > limit,
    }
  }

  async getIntegrationHealth(): Promise<IntegrationHealthReport> {
    const adapters: IntegrationHealthEntry[] = PREVIEW_ADAPTERS.map((capabilities) => {
      const status = integrationStatusForAdapter(capabilities)
      return {
        source: capabilities.source,
        status,
        capabilities,
        detail: status === 'healthy' ? 'Preview adapter ready' : 'Preview adapter limited',
      }
    })

    return {
      checkedAtMs: Date.now(),
      overall: summarizeIntegrationHealth(adapters),
      adapters,
    }
  }

  async previewConnector(source: ConnectorPreview['source']): Promise<ConnectorPreview> {
    return {
      planId: `preview-${source}`,
      source,
      summary: 'Preview only — no vendor configuration files were changed.',
      expiresAtMs: Date.now() + 300_000,
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

/** Ensures preview clients never masquerade as production hosts. */
export function assertPreviewClient(client: NativeClient): asserts client is FakeNativeClient {
  if (client.mode !== 'preview') {
    throw new NativeClientError('not-available', 'Expected preview native client')
  }
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
