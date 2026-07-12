import type { AppSnapshot, DecisionRequest } from '../../native/contracts'

export type OverlayMode = 'compact' | 'peek'

export type OverlayRenderContext = 'preview' | 'native'

export type OverlayPlatform = 'macos' | 'windows'

export type OverlayConnectionState =
  | 'live'
  | 'empty'
  | 'stale'
  | 'warmingUp'
  | 'metricsUnavailable'
  | 'ipcError'
  | 'coreError'

export type HealthBeaconTone = 'healthy' | 'attention' | 'degraded' | 'error'

export interface OverlayCpuSample {
  atMs: number
  cpuCorePercent: number
}

export interface OverlayShellProps {
  mode: OverlayMode
  renderContext: OverlayRenderContext
  platform: OverlayPlatform
  reducedMotion: boolean
  connectionState: OverlayConnectionState
  snapshot?: AppSnapshot | undefined
  cpuHistory?: readonly OverlayCpuSample[] | undefined
  staleMessage?: string | undefined
  errorMessage?: string | undefined
  nowMs?: number | undefined
  onOpenDashboard?: (() => void) | undefined
  onAcknowledge?: ((sessionId: string) => void) | undefined
  pendingDecision?: DecisionRequest | undefined
  decisionControlsEnabled?: boolean | undefined
  onDecisionAllow?: (() => void) | undefined
  onDecisionDeny?: (() => void) | undefined
  emptyMessage?: string | null | undefined
}
