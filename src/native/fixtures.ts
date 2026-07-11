import type {
  AdapterCapabilities,
  AgentSession,
  AppSnapshot,
  HostMetricSample,
  MetricSample,
  MetricsFrame,
  PublicSettings,
  SessionEvent,
} from './contracts.ts'
import { PROTOCOL_VERSION } from './contracts.ts'

export const DEFAULT_PUBLIC_SETTINGS: PublicSettings = {
  overlayEnabled: true,
  autostartEnabled: false,
  reducedMotion: false,
  samplingIntervalMs: 1_000,
  showOverFullscreen: false,
  historyRetentionHours: 24,
}

export const PREVIEW_ADAPTERS: AdapterCapabilities[] = [
  {
    source: 'cursor',
    events: true,
    attention: 'none',
    decisionResponse: false,
    contextOpen: false,
    processAttribution: 'unknown',
  },
  {
    source: 'claudeCode',
    events: true,
    attention: 'partial',
    decisionResponse: false,
    contextOpen: false,
    processAttribution: 'unknown',
  },
  {
    source: 'codex',
    events: true,
    attention: 'none',
    decisionResponse: false,
    contextOpen: false,
    processAttribution: 'unknown',
  },
]

const now = Date.now()

function metricSample(overrides: Partial<MetricSample> = {}): MetricSample {
  return {
    atMs: now,
    cpuCorePercent: 12.4,
    cpuHostPercent: 4.2,
    rssBytes: 412_000_000,
    runtimeMs: 182_000,
    processCount: 3,
    readBytesPerSec: 120_000,
    writeBytesPerSec: 48_000,
    quality: {
      attribution: 'unknown',
      cpu: 'available',
      io: 'disk',
    },
    ...overrides,
  }
}

export const PREVIEW_SESSIONS: AgentSession[] = [
  {
    id: 'sess-cursor-refactor',
    source: 'cursor',
    externalSessionId: 'cursor:workspace-alpha',
    label: 'Refactor telemetry bridge',
    workspaceLabel: 'llm_notch',
    status: 'running',
    attention: 'none',
    startedAtMs: now - 420_000,
    lastEventAtMs: now - 4_000,
    latestMetric: metricSample({
      cpuCorePercent: 38.5,
      cpuHostPercent: 9.8,
      rssBytes: 612_000_000,
      processCount: 5,
    }),
  },
  {
    id: 'sess-claude-review',
    source: 'claudeCode',
    externalSessionId: 'claude:review-42',
    label: 'Review overlay accessibility',
    workspaceLabel: 'llm_notch',
    status: 'waiting',
    attention: 'approval',
    startedAtMs: now - 180_000,
    lastEventAtMs: now - 12_000,
    latestMetric: metricSample({
      cpuCorePercent: 6.2,
      cpuHostPercent: 1.4,
      rssBytes: 248_000_000,
      processCount: 2,
    }),
  },
  {
    id: 'sess-codex-docs',
    source: 'codex',
    externalSessionId: 'codex:docs-pass',
    label: 'Draft connector setup notes',
    workspaceLabel: 'llm_notch',
    status: 'completed',
    attention: 'none',
    startedAtMs: now - 900_000,
    lastEventAtMs: now - 60_000,
    endedAtMs: now - 45_000,
    latestMetric: metricSample({
      cpuCorePercent: 0,
      cpuHostPercent: 0,
      rssBytes: 0,
      processCount: 0,
      readBytesPerSec: 0,
      writeBytesPerSec: 0,
      quality: {
        attribution: 'unknown',
        cpu: 'unavailable',
        io: 'unavailable',
        reason: 'Session completed',
      },
    }),
  },
]

export const PREVIEW_EVENTS: SessionEvent[] = [
  {
    id: 'evt-1',
    sessionId: 'sess-cursor-refactor',
    sequence: 1,
    occurredAtMs: now - 300_000,
    kind: 'lifecycle',
    level: 'info',
    summary: 'Session started from Cursor hook',
  },
  {
    id: 'evt-2',
    sessionId: 'sess-cursor-refactor',
    sequence: 2,
    occurredAtMs: now - 120_000,
    kind: 'tool',
    level: 'info',
    summary: 'Read src/native/contracts.ts',
    toolName: 'read',
  },
  {
    id: 'evt-3',
    sessionId: 'sess-claude-review',
    sequence: 1,
    occurredAtMs: now - 90_000,
    kind: 'attention',
    level: 'warning',
    summary: 'Approval required before editing accessibility copy',
  },
  {
    id: 'evt-4',
    sessionId: 'sess-codex-docs',
    sequence: 1,
    occurredAtMs: now - 80_000,
    kind: 'lifecycle',
    level: 'info',
    summary: 'Session completed successfully',
  },
]

export function createPreviewHostMetrics(atMs = Date.now()): HostMetricSample {
  return {
    atMs,
    cpuHostPercent: 18.5,
    usedMemoryBytes: 12_400_000_000,
    totalMemoryBytes: 17_100_000_000,
    visibleProcessCount: 214,
    diskReadBytesPerSec: 2_400_000,
    diskWriteBytesPerSec: 980_000,
  }
}

export function createPreviewMetricsFrame(atMs = Date.now()): MetricsFrame {
  const host = createPreviewHostMetrics(atMs)

  return {
    host,
    aggregate: {
      atMs,
      cpuCorePercent: 44.7,
      cpuHostPercent: 11.2,
      rssBytes: 860_000_000,
      runtimeMs: 602_000,
      processCount: 7,
      readBytesPerSec: 360_000,
      writeBytesPerSec: 140_000,
      quality: {
        attribution: 'unknown',
        cpu: 'available',
        io: 'disk',
      },
      activeSessions: 2,
      attentionSessions: 1,
    },
    agents: {
      'sess-cursor-refactor': metricSample({
        atMs,
        cpuCorePercent: 38.5 + Math.sin(atMs / 4_000) * 4,
        cpuHostPercent: 9.8,
        rssBytes: 612_000_000 + Math.round(Math.sin(atMs / 5_000) * 8_000_000),
      }),
      'sess-claude-review': metricSample({
        atMs,
        cpuCorePercent: 6.2 + Math.cos(atMs / 6_000) * 1.5,
        cpuHostPercent: 1.4,
        rssBytes: 248_000_000,
      }),
    },
  }
}

export function createPreviewSnapshot(atMs = Date.now()): AppSnapshot {
  const metrics = createPreviewMetricsFrame(atMs)

  return {
    protocolVersion: PROTOCOL_VERSION,
    capturedAtMs: atMs,
    host: metrics.host,
    aggregate: metrics.aggregate,
    sessions: PREVIEW_SESSIONS.map((session) => {
      const latestMetric = metrics.agents[session.id] ?? session.latestMetric
      return latestMetric ? { ...session, latestMetric } : session
    }),
    settings: DEFAULT_PUBLIC_SETTINGS,
    adapters: PREVIEW_ADAPTERS,
  }
}
