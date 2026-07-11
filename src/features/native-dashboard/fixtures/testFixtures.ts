import type {
  AdapterCapabilities,
  AgentSession,
  MetricQuality,
  MetricSample,
  PublicSettings,
  SessionEvent,
} from '../../../native/contracts'
import type { IntegrationCardState, MetricsHistoryBundle } from '../types/contracts'

const now = 1_700_000_000_000

function metricQuality(overrides: Partial<MetricQuality> = {}): MetricQuality {
  return {
    attribution: 'exact',
    cpu: 'available',
    io: 'disk',
    ...overrides,
  }
}

function metricSample(overrides: Partial<MetricSample> = {}): MetricSample {
  return {
    atMs: now,
    cpuCorePercent: 12.4,
    cpuHostPercent: 4.2,
    rssBytes: 512 * 1024 * 1024,
    runtimeMs: 1_800_000,
    processCount: 3,
    readBytesPerSec: 120_000,
    writeBytesPerSec: 45_000,
    quality: metricQuality(),
    ...overrides,
  }
}

export const mockSessions: AgentSession[] = [
  {
    id: 'sess-cursor-1',
    source: 'cursor',
    externalSessionId: 'cursor-abc',
    label: 'Refactor auth middleware',
    workspaceLabel: 'auth-service',
    status: 'running',
    attention: 'approval',
    startedAtMs: now - 1_800_000,
    lastEventAtMs: now - 30_000,
    processRoot: { pid: 4412, startedAtMs: now - 1_800_000 },
    latestMetric: metricSample(),
  },
  {
    id: 'sess-claude-1',
    source: 'claudeCode',
    externalSessionId: 'claude-xyz',
    label: 'Write integration tests',
    workspaceLabel: 'qa-harness',
    status: 'waiting',
    attention: 'question',
    startedAtMs: now - 3_600_000,
    lastEventAtMs: now - 120_000,
    latestMetric: metricSample({
      cpuCorePercent: 6.1,
      rssBytes: 280 * 1024 * 1024,
      processCount: 2,
    }),
  },
  {
    id: 'sess-codex-1',
    source: 'codex',
    externalSessionId: 'codex-123',
    label: 'Docs cleanup',
    workspaceLabel: 'docs',
    status: 'completed',
    attention: 'none',
    startedAtMs: now - 7_200_000,
    lastEventAtMs: now - 3_600_000,
    endedAtMs: now - 3_600_000,
    latestMetric: metricSample({
      cpuCorePercent: 0,
      processCount: 0,
      quality: metricQuality({ cpu: 'unavailable', io: 'unavailable' }),
    }),
  },
]

export const mockEvents: SessionEvent[] = [
  {
    id: 'evt-1',
    sessionId: 'sess-cursor-1',
    sequence: 1,
    occurredAtMs: now - 600_000,
    kind: 'lifecycle',
    level: 'info',
    summary: 'Session started',
  },
  {
    id: 'evt-2',
    sessionId: 'sess-cursor-1',
    sequence: 2,
    occurredAtMs: now - 120_000,
    kind: 'tool',
    level: 'info',
    summary: 'Ran cargo test',
    toolName: 'shell',
  },
  {
    id: 'evt-3',
    sessionId: 'sess-cursor-1',
    sequence: 3,
    occurredAtMs: now - 30_000,
    kind: 'attention',
    level: 'warning',
    summary: 'Approval required for destructive command',
  },
]

export const mockAdapters: AdapterCapabilities[] = [
  {
    source: 'cursor',
    events: true,
    attention: 'partial',
    decisionResponse: false,
    contextOpen: true,
    processAttribution: 'exact',
  },
  {
    source: 'claudeCode',
    events: true,
    attention: 'full',
    decisionResponse: false,
    contextOpen: true,
    processAttribution: 'shared',
  },
  {
    source: 'codex',
    events: true,
    attention: 'none',
    decisionResponse: false,
    contextOpen: false,
    processAttribution: 'heuristic',
  },
  {
    source: 'generic',
    events: false,
    attention: 'none',
    decisionResponse: false,
    contextOpen: false,
    processAttribution: 'unknown',
  },
]

export const mockSettings: PublicSettings = {
  overlayEnabled: true,
  autostartEnabled: false,
  reducedMotion: false,
  samplingIntervalMs: 2000,
  selectedDisplay: 'display-primary',
  showOverFullscreen: false,
  historyRetentionHours: 24,
}

export const mockDisplays = [
  { id: 'display-primary', label: 'Built-in Display' },
  { id: 'display-external', label: 'External Monitor' },
]

export const mockIntegrations: IntegrationCardState[] = mockAdapters
  .filter((adapter) => adapter.source !== 'generic')
  .map((adapter) => {
    const card: IntegrationCardState = {
      adapter,
      status: adapter.source === 'generic' ? 'notInstalled' : 'waitingFirstEvent',
      managedEntriesPresent: adapter.source === 'cursor',
    }

    if (adapter.events) {
      card.lastEventAtMs = now - 45_000
    }

    return card
  })

export function buildHistory(points = 12): MetricsHistoryBundle {
  const requestedStartMs = now - 24 * 60 * 60_000
  const requestedEndMs = now
  const hostCpu = Array.from({ length: points }, (_, index) => ({
    atMs: now - (points - index) * 60_000,
    value: 8 + Math.sin(index / 2) * 3,
  }))
  const aggregateCpu = Array.from({ length: points }, (_, index) => ({
    atMs: now - (points - index) * 60_000,
    value: 18 + Math.cos(index / 3) * 5,
  }))
  const aggregateRss = Array.from({ length: points }, (_, index) => ({
    atMs: now - (points - index) * 60_000,
    value: 700 + index * 12,
  }))
  const firstHistoryPoint = hostCpu[0]
  const lastHistoryPoint = hostCpu.at(-1)
  const coverage = {
    requestedStartMs,
    requestedEndMs,
    ...(firstHistoryPoint ? { actualFirstMs: firstHistoryPoint.atMs } : {}),
    ...(lastHistoryPoint ? { actualLastMs: lastHistoryPoint.atMs } : {}),
    totalPoints: points,
    returnedPoints: points,
    downsampled: false,
    truncated: false,
  }

  return {
    requestedStartMs,
    requestedEndMs,
    hostCpu,
    aggregateCpu,
    aggregateRss,
    hostCoverage: coverage,
    aggregateCoverage: coverage,
    perAgent: [
      {
        sessionId: 'cursor-session',
        source: 'cursor',
        label: 'Cursor',
        cpu: aggregateCpu,
        rss: aggregateRss,
        coverage,
      },
      {
        sessionId: 'claude-session',
        source: 'claudeCode',
        label: 'Claude Code',
        cpu: hostCpu,
        rss: aggregateRss.map((point) => ({ ...point, value: point.value * 0.6 })),
        coverage,
      },
    ],
  }
}

export const mockHost = {
  atMs: now,
  cpuHostPercent: 22.5,
  usedMemoryBytes: 12 * 1024 * 1024 * 1024,
  totalMemoryBytes: 32 * 1024 * 1024 * 1024,
  visibleProcessCount: 142,
  diskReadBytesPerSec: 2_400_000,
  diskWriteBytesPerSec: 980_000,
}

export const mockAggregate = {
  atMs: now,
  cpuCorePercent: 24.8,
  cpuHostPercent: 9.1,
  rssBytes: 1_100 * 1024 * 1024,
  runtimeMs: 5_400_000,
  processCount: 8,
  readBytesPerSec: 310_000,
  writeBytesPerSec: 140_000,
  quality: metricQuality({ io: 'allIo' }),
  activeSessions: 2,
  attentionSessions: 2,
}

export const mockAgentMetrics: Record<string, MetricSample> = {
  cursor: metricSample(),
  claudeCode: metricSample({
    cpuCorePercent: 8.2,
    rssBytes: 420 * 1024 * 1024,
    quality: metricQuality({ attribution: 'shared' }),
  }),
}

export const FIXED_NOW_MS = now
