import { Channel, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { NATIVE_COMMANDS, NATIVE_EVENTS } from './commands.ts'
import type {
  AdapterCapabilities,
  AgentCatalogEntry,
  AppSnapshot,
  BackupJournalEntry,
  ConnectorApplyResult,
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
  SessionEvent,
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
import type {
  BootstrapResult,
  ConnectorPreview,
  IntegrationHealthReport,
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
  private remoteConnectionUnlisten: (() => void) | null = null
  private connectorHealthUnlisten: (() => void) | null = null

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

  async subscribeRemoteConnectionChanges(
    onChange: RemoteConnectionChangeHandler,
  ): Promise<RemoteConnectionSubscription> {
    if (this.remoteConnectionUnlisten) {
      throw new NativeClientError(
        'not-available',
        'Remote connection subscription is already active',
      )
    }

    try {
      const unlisten = await listen<RemoteConnectionStatusView>(
        NATIVE_EVENTS.remoteConnectionChanged,
        (event) => {
          onChange(event.payload)
        },
      )
      this.remoteConnectionUnlisten = unlisten

      return {
        unsubscribe: async () => {
          await this.unsubscribeRemoteConnectionChanges()
        },
      }
    } catch (error) {
      throw toNativeError(error, 'Failed to subscribe to remote connection changes')
    }
  }

  async subscribeConnectorHealthChanges(
    onChange: ConnectorHealthChangeHandler,
  ): Promise<ConnectorHealthSubscription> {
    if (this.connectorHealthUnlisten) {
      throw new NativeClientError(
        'not-available',
        'Connector health subscription is already active',
      )
    }

    try {
      const unlisten = await listen(NATIVE_EVENTS.connectorHealthChanged, () => {
        onChange()
      })
      this.connectorHealthUnlisten = unlisten

      return {
        unsubscribe: async () => {
          await this.unsubscribeConnectorHealthChanges()
        },
      }
    } catch (error) {
      throw toNativeError(error, 'Failed to subscribe to connector health changes')
    }
  }

  async openDashboard(): Promise<void> {
    await this.invokeVoid(NATIVE_COMMANDS.openDashboard)
  }

  async openSession(sessionId: string): Promise<OpenSessionResult> {
    try {
      return await invoke<OpenSessionResult>(NATIVE_COMMANDS.openSession, { sessionId })
    } catch (error) {
      throw toNativeError(error, 'Failed to open session context')
    }
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

  async listAgentCatalog(): Promise<AgentCatalogEntry[]> {
    try {
      return await invoke<AgentCatalogEntry[]>(NATIVE_COMMANDS.listAgentCatalog)
    } catch (error) {
      throw toNativeError(error, 'Failed to load agent catalog')
    }
  }

  async listQuotaSnapshots(): Promise<QuotaSnapshotView[]> {
    try {
      return await invoke<QuotaSnapshotView[]>(NATIVE_COMMANDS.listQuotaSnapshots)
    } catch (error) {
      throw toNativeError(error, 'Failed to load quota snapshots')
    }
  }

  async listRemoteHosts(): Promise<RemoteHostView[]> {
    try {
      return await invoke<RemoteHostView[]>(NATIVE_COMMANDS.listRemoteHosts)
    } catch (error) {
      throw toNativeError(error, 'Failed to load remote hosts')
    }
  }

  async upsertRemoteHost(config: RemoteHostConfigInput): Promise<RemoteHostView> {
    try {
      return await invoke<RemoteHostView>(NATIVE_COMMANDS.upsertRemoteHost, { config })
    } catch (error) {
      throw toNativeError(error, 'Failed to save remote host')
    }
  }

  async removeRemoteHost(hostId: string): Promise<void> {
    try {
      await invoke(NATIVE_COMMANDS.removeRemoteHost, { hostId })
    } catch (error) {
      throw toNativeError(error, 'Failed to remove remote host')
    }
  }

  async getRemoteBackendStatus(): Promise<RemoteBackendStatus> {
    try {
      return await invoke<RemoteBackendStatus>(NATIVE_COMMANDS.getRemoteBackendStatus)
    } catch (error) {
      throw toNativeError(error, 'Failed to load remote backend status')
    }
  }

  async previewRemoteDeploy(hostId: string): Promise<RemoteDeploymentPlanView> {
    try {
      return await invoke<RemoteDeploymentPlanView>(NATIVE_COMMANDS.previewRemoteDeploy, {
        hostId,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to preview remote deployment')
    }
  }

  async executeRemoteDeploy(hostId: string): Promise<RemoteDeploymentResultView> {
    try {
      return await invoke<RemoteDeploymentResultView>(NATIVE_COMMANDS.executeRemoteDeploy, {
        hostId,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to execute remote deployment')
    }
  }

  async startRemoteRelay(hostId: string): Promise<RemoteConnectionStatusView> {
    try {
      return await invoke<RemoteConnectionStatusView>(NATIVE_COMMANDS.startRemoteRelay, {
        hostId,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to start remote relay')
    }
  }

  async stopRemoteRelay(hostId: string): Promise<RemoteConnectionStatusView> {
    try {
      return await invoke<RemoteConnectionStatusView>(NATIVE_COMMANDS.stopRemoteRelay, { hostId })
    } catch (error) {
      throw toNativeError(error, 'Failed to stop remote relay')
    }
  }

  async getRemoteConnectionStatus(hostId: string): Promise<RemoteConnectionStatusView> {
    try {
      return await invoke<RemoteConnectionStatusView>(NATIVE_COMMANDS.getRemoteConnectionStatus, {
        hostId,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to load remote connection status')
    }
  }

  async getSoundThemes(): Promise<SoundTheme[]> {
    try {
      return await invoke<SoundTheme[]>(NATIVE_COMMANDS.getSoundThemes)
    } catch (error) {
      throw toNativeError(error, 'Failed to load sound themes')
    }
  }

  async previewSoundRouting(request: {
    routing: SoundRouting
    event: SoundEvent
    agent?: string
    localMinute: number
  }): Promise<SoundRoutingPreview> {
    try {
      return await invoke<SoundRoutingPreview>(NATIVE_COMMANDS.previewSoundRouting, { request })
    } catch (error) {
      throw toNativeError(error, 'Failed to preview sound routing')
    }
  }

  async playSoundEvent(request: SoundPlayRequest): Promise<SoundPlayResult> {
    try {
      return await invoke<SoundPlayResult>(NATIVE_COMMANDS.playSoundEvent, { request })
    } catch (error) {
      throw toNativeError(error, 'Failed to play sound event')
    }
  }

  async importSoundPack(request: ImportSoundPackRequest): Promise<SoundPackValidation> {
    try {
      return await invoke<SoundPackValidation>(NATIVE_COMMANDS.importSoundPack, { request })
    } catch (error) {
      throw toNativeError(error, 'Failed to import sound pack')
    }
  }

  async previewConnector(
    source: ConnectorPreview['source'],
    scope: ConnectorScope = 'user',
  ): Promise<ConnectorPreview> {
    try {
      return await invoke<ConnectorPreview>(NATIVE_COMMANDS.previewConnector, { source, scope })
    } catch (error) {
      throw toNativeError(error, 'Failed to preview connector template')
    }
  }

  async detectConnectors(): Promise<DetectedConnector[]> {
    try {
      return await invoke<DetectedConnector[]>(NATIVE_COMMANDS.detectConnectors)
    } catch (error) {
      throw toNativeError(error, 'Failed to detect connectors')
    }
  }

  async applyConnectorChange(
    planId: string,
    selectedDisplayPaths?: string[],
  ): Promise<ConnectorApplyResult> {
    try {
      return await invoke<ConnectorApplyResult>(NATIVE_COMMANDS.applyConnector, {
        planId,
        selectedDisplayPaths,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to apply connector change')
    }
  }

  async removeConnector(
    source: AdapterCapabilities['source'],
    scope: ConnectorScope = 'user',
  ): Promise<ConnectorApplyResult> {
    try {
      return await invoke<ConnectorApplyResult>(NATIVE_COMMANDS.removeConnector, { source, scope })
    } catch (error) {
      throw toNativeError(error, 'Failed to remove connector')
    }
  }

  async repairConnector(
    source: AdapterCapabilities['source'],
    scope: ConnectorScope = 'user',
  ): Promise<ConnectorPlanPreview> {
    try {
      return await invoke<ConnectorPlanPreview>(NATIVE_COMMANDS.repairConnector, { source, scope })
    } catch (error) {
      throw toNativeError(error, 'Failed to preview connector repair')
    }
  }

  async rollbackConnector(backupId: string): Promise<ConnectorPlanPreview> {
    try {
      return await invoke<ConnectorPlanPreview>(NATIVE_COMMANDS.rollbackConnector, { backupId })
    } catch (error) {
      throw toNativeError(error, 'Failed to preview connector rollback')
    }
  }

  async listConnectorBackups(): Promise<BackupJournalEntry[]> {
    try {
      return await invoke<BackupJournalEntry[]>(NATIVE_COMMANDS.listConnectorBackups)
    } catch (error) {
      throw toNativeError(error, 'Failed to list connector backups')
    }
  }

  async getPendingDecisions(): Promise<DecisionRequest[]> {
    try {
      return await invoke<DecisionRequest[]>(NATIVE_COMMANDS.getPendingDecisions)
    } catch (error) {
      throw toNativeError(error, 'Failed to load pending decisions')
    }
  }

  async respondDecision(
    requestId: string,
    response: DecisionResponse,
  ): Promise<DecisionResponseRecord> {
    try {
      return await invoke<DecisionResponseRecord>(NATIVE_COMMANDS.submitDecision, {
        requestId,
        response,
      })
    } catch (error) {
      throw toNativeError(error, 'Failed to submit decision response')
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

  private async unsubscribeRemoteConnectionChanges(): Promise<void> {
    const unlisten = this.remoteConnectionUnlisten
    this.remoteConnectionUnlisten = null
    unlisten?.()
  }

  private async unsubscribeConnectorHealthChanges(): Promise<void> {
    const unlisten = this.connectorHealthUnlisten
    this.connectorHealthUnlisten = null
    unlisten?.()
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
