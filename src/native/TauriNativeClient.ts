import { Channel, invoke } from '@tauri-apps/api/core'
import { NATIVE_COMMANDS } from './commands.ts'
import type { AppSnapshot, PublicSettings, SessionEvent, StreamFrame } from './contracts.ts'
import { PROTOCOL_VERSION } from './contracts.ts'
import { NativeClientError } from './errors.ts'
import type {
  BootstrapResult,
  ConnectorPreview,
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

interface BootstrapResponse {
  snapshot: AppSnapshot
  lastSequence?: number
  events?: SessionEvent[]
}

function toNativeError(error: unknown, fallback: string): NativeClientError {
  if (error instanceof NativeClientError) {
    return error
  }

  if (error instanceof Error) {
    return new NativeClientError('invoke-failed', error.message, error)
  }

  return new NativeClientError('invoke-failed', fallback, error)
}

function assertProtocolVersion(version: number): void {
  if (version !== PROTOCOL_VERSION) {
    throw new NativeClientError(
      'protocol-incompatible',
      `Host protocol ${version} is incompatible with renderer protocol ${PROTOCOL_VERSION}`,
    )
  }
}

export class TauriNativeClient implements NativeClient {
  readonly mode = 'tauri' as const

  private channel: Channel<StreamFrame> | null = null
  private subscriptionId: string | null = null
  private lastSequence: number | null = null
  private onFrame: StreamFrameHandler | null = null
  private onError: StreamErrorHandler | null = null

  async bootstrap(): Promise<BootstrapResult> {
    try {
      const response = await invoke<BootstrapResponse>(NATIVE_COMMANDS.bootstrap)
      assertProtocolVersion(response.snapshot.protocolVersion)

      const lastSequence = response.lastSequence ?? 0
      this.lastSequence = lastSequence

      return {
        snapshot: response.snapshot,
        lastSequence,
        events: response.events ?? [],
      }
    } catch (error) {
      throw toNativeError(error, 'Failed to bootstrap native snapshot')
    }
  }

  async subscribe(
    onFrame: StreamFrameHandler,
    onError: StreamErrorHandler,
  ): Promise<StreamSubscription> {
    if (this.channel) {
      throw new NativeClientError('not-available', 'Native stream subscription is already active')
    }

    this.onFrame = onFrame
    this.onError = onError

    const channel = new Channel<StreamFrame>()
    this.channel = channel

    channel.onmessage = (frame) => {
      this.handleIncomingFrame(frame)
    }

    try {
      const subscriptionId = await invoke<string>(NATIVE_COMMANDS.subscribeStream, {
        afterSequence: this.lastSequence ?? 0,
        onEvent: channel,
      })
      this.subscriptionId = subscriptionId
    } catch (error) {
      this.channel = null
      this.onFrame = null
      this.onError = null
      throw toNativeError(error, 'Failed to subscribe to native stream')
    }

    return {
      unsubscribe: async () => {
        await this.unsubscribe()
      },
    }
  }

  async openDashboard(): Promise<void> {
    await this.invokeVoid(NATIVE_COMMANDS.openDashboard)
  }

  async openSession(sessionId: string): Promise<void> {
    await this.invokeVoid(NATIVE_COMMANDS.openSession, { sessionId })
  }

  async setOverlayMode(mode: OverlayMode): Promise<void> {
    await this.invokeVoid(NATIVE_COMMANDS.setOverlayMode, { mode })
  }

  async acknowledgeLocalAttention(sessionId: string): Promise<void> {
    await this.invokeVoid(NATIVE_COMMANDS.acknowledgeAttention, { sessionId })
  }

  async updateSettings(settings: PublicSettings): Promise<PublicSettings> {
    try {
      return await invoke<PublicSettings>(NATIVE_COMMANDS.updateSettings, { settings })
    } catch (error) {
      throw toNativeError(error, 'Failed to update settings')
    }
  }

  async purgeHistory(scope?: import('./contracts.ts').PurgeScope): Promise<void> {
    await this.invokeVoid(NATIVE_COMMANDS.purgeHistory, { scope })
  }

  async getHistory(range: NativeHistoryRange): Promise<NativeHistoryResponse> {
    try {
      return await invoke<NativeHistoryResponse>(NATIVE_COMMANDS.getHistory, { range })
    } catch (error) {
      throw toNativeError(error, `Failed to load ${range} metrics history`)
    }
  }

  async listDisplays(): Promise<NativeDisplayOption[]> {
    try {
      return await invoke<NativeDisplayOption[]>(NATIVE_COMMANDS.listDisplays)
    } catch (error) {
      throw toNativeError(error, 'Failed to enumerate native displays')
    }
  }

  async getSessionEvents(
    sessionId: string,
    beforeSequence?: number,
    limit = 50,
  ): Promise<SessionEventPage> {
    try {
      return await invoke<SessionEventPage>(NATIVE_COMMANDS.getSessionEvents, {
        sessionId,
        ...(beforeSequence === undefined ? {} : { beforeSequence }),
        limit,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to load session event history')
    }
  }

  async getIntegrationHealth(): Promise<IntegrationHealthReport> {
    try {
      return await invoke<IntegrationHealthReport>(NATIVE_COMMANDS.integrationHealth)
    } catch (error) {
      throw toNativeError(error, 'Failed to fetch integration health')
    }
  }

  async previewConnector(source: ConnectorPreview['source']): Promise<ConnectorPreview> {
    try {
      return await invoke<ConnectorPreview>(NATIVE_COMMANDS.previewConnector, { source })
    } catch (error) {
      throw toNativeError(error, 'Failed to preview connector template')
    }
  }

  private async invokeVoid(command: string, args?: Record<string, unknown>): Promise<void> {
    try {
      await invoke(command, args)
    } catch (error) {
      throw toNativeError(error, `Native command "${command}" failed`)
    }
  }

  private handleIncomingFrame(frame: StreamFrame): void {
    if (frame.payload.type === 'resyncRequired') {
      this.onError?.(
        new NativeClientError(
          'resync-required',
          frame.payload.reason || 'Native stream requested resync',
        ),
      )
      return
    }

    // Sequence validation lives in NativeStateProvider so both the Tauri and
    // preview clients follow the same rules without dropping intermediate IDs.
    this.lastSequence = frame.sequence
    this.onFrame?.(frame)
  }

  private async unsubscribe(): Promise<void> {
    this.onFrame = null
    this.onError = null
    this.lastSequence = null

    const subscriptionId = this.subscriptionId
    this.subscriptionId = null
    this.channel = null

    if (!subscriptionId) {
      return
    }

    try {
      await invoke(NATIVE_COMMANDS.unsubscribeStream, { subscriptionId })
    } catch (error) {
      throw toNativeError(error, 'Failed to unsubscribe from native stream')
    }
  }
}

export function createTauriNativeClient(): TauriNativeClient {
  return new TauriNativeClient()
}
