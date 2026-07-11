import type {
  AgentSession,
  AppSnapshot,
  MetricQuality,
  MetricSample,
  PublicSettings,
} from '../../../native/contracts'
import type { OverlayCpuSample, OverlayShellProps } from '../types'

const defaultSettings: PublicSettings = {
  overlayEnabled: true,
  autostartEnabled: false,
  reducedMotion: false,
  samplingIntervalMs: 1000,
  showOverFullscreen: false,
  historyRetentionHours: 24,
}

const defaultQuality: MetricQuality = {
  attribution: 'exact',
  cpu: 'available',
  io: 'disk',
}

export function createMetricSample(overrides: Partial<MetricSample> = {}): MetricSample {
  return {
    atMs: 1_700_000_000_000,
    cpuCorePercent: 42,
    cpuHostPercent: 12,
    rssBytes: 256 * 1024 * 1024,
    runtimeMs: 120_000,
    processCount: 2,
    readBytesPerSec: 4096,
    writeBytesPerSec: 8192,
    quality: defaultQuality,
    ...overrides,
  }
}

export function createSession(overrides: Partial<AgentSession> = {}): AgentSession {
  return {
    id: 'session-1',
    source: 'cursor',
    externalSessionId: 'ext-1',
    label: 'Refactor auth flow',
    workspaceLabel: 'auth-service',
    status: 'running',
    attention: 'none',
    startedAtMs: 1_700_000_000_000 - 120_000,
    lastEventAtMs: 1_700_000_000_000,
    latestMetric: createMetricSample(),
    ...overrides,
  }
}

export function createSnapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  const sessions = overrides.sessions ?? [
    createSession(),
    createSession({
      id: 'session-2',
      source: 'claudeCode',
      label: 'Write tests',
      attention: 'approval',
      status: 'waiting',
      lastEventAtMs: 1_700_000_000_500,
    }),
    createSession({
      id: 'session-3',
      source: 'codex',
      label: 'Review API',
      attention: 'question',
      status: 'waiting',
      lastEventAtMs: 1_700_000_000_400,
    }),
  ]

  return {
    protocolVersion: 1,
    capturedAtMs: 1_700_000_000_000,
    host: {
      atMs: 1_700_000_000_000,
      cpuHostPercent: 18,
      usedMemoryBytes: 8 * 1024 ** 3,
      totalMemoryBytes: 16 * 1024 ** 3,
      visibleProcessCount: 120,
      diskReadBytesPerSec: 1024,
      diskWriteBytesPerSec: 2048,
    },
    aggregate: {
      atMs: 1_700_000_000_000,
      cpuCorePercent: 88,
      cpuHostPercent: 18,
      rssBytes: 768 * 1024 * 1024,
      runtimeMs: 240_000,
      processCount: 5,
      readBytesPerSec: 12_288,
      writeBytesPerSec: 24_576,
      quality: {
        attribution: 'shared',
        cpu: 'available',
        io: 'allIo',
      },
      activeSessions: sessions.length,
      attentionSessions: sessions.filter((session) => session.attention !== 'none').length,
    },
    sessions,
    settings: defaultSettings,
    adapters: [],
    ...overrides,
  }
}

export function createCpuHistory(nowMs = 1_700_000_000_000): OverlayCpuSample[] {
  return Array.from({ length: 12 }, (_, index) => ({
    atMs: nowMs - (11 - index) * 2500,
    cpuCorePercent: 20 + index * 5,
  }))
}

export function createOverlayProps(overrides: Partial<OverlayShellProps> = {}): OverlayShellProps {
  const nowMs = overrides.nowMs ?? 1_700_000_000_000
  return {
    mode: 'compact',
    renderContext: 'native',
    platform: 'macos',
    reducedMotion: false,
    connectionState: 'live',
    snapshot: createSnapshot(),
    cpuHistory: createCpuHistory(nowMs),
    nowMs,
    ...overrides,
  }
}
