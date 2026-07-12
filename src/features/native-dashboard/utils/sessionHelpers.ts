import type {
  AdapterCapabilities,
  AgentSession,
  AgentSource,
  ConnectorUserStatus,
  DecisionRequest,
  DetectedConnector,
} from '../../../native/contracts'
import type { IntegrationHealthReport } from '../../../native/types'
import { agentLabel } from './formatters'
import { bestDetectedConnector } from './integrationLabels'

const TRACKED_SOURCES = new Set<AgentSource>([
  'cursor',
  'claudeCode',
  'codex',
  'gemini',
  'qwen',
  'antigravityCli',
  'copilotCli',
])

function connectorStatus(
  health: IntegrationHealthReport | null | undefined,
  source: AgentSource,
): ConnectorUserStatus | undefined {
  return health?.adapters.find((entry) => entry.source === source)?.status
}

function trackedAdapters(adapters: AdapterCapabilities[]): AdapterCapabilities[] {
  return adapters.filter((adapter) => TRACKED_SOURCES.has(adapter.source))
}

function formatAgentList(sources: AgentSource[]): string {
  const labels = sources.map((source) => agentLabel(source))
  if (labels.length <= 2) {
    return labels.join(' and ')
  }
  return `${labels.slice(0, -1).join(', ')}, and ${labels.at(-1)}`
}

export function deriveSessionsEmptyMessage(
  adapters: AdapterCapabilities[],
  health: IntegrationHealthReport | null | undefined,
  detectedConnectors: DetectedConnector[] = [],
): string {
  const tracked = trackedAdapters(adapters)

  if (tracked.length === 0) {
    return 'No agent sessions yet. Connect an integration in the dashboard to start tracking live sessions.'
  }

  const waitingForTraffic = tracked.filter((adapter) => {
    const status = connectorStatus(health, adapter.source)
    return status === 'waitingFirstEvent' || status === 'connected'
  })
  if (waitingForTraffic.length > 0) {
    return `Hooks are installed for ${formatAgentList(waitingForTraffic.map((adapter) => adapter.source))}. Start or resume an agent session to verify live traffic.`
  }

  const processWithoutHooks = tracked.filter((adapter) => {
    const detected = bestDetectedConnector(detectedConnectors, adapter.source)
    return Boolean(detected?.processRunning) && !detected?.managedEntriesPresent
  })
  if (processWithoutHooks.length > 0) {
    return `Agent processes are running for ${formatAgentList(processWithoutHooks.map((adapter) => adapter.source))}, but hooks have not verified a session yet. Check Integrations in the dashboard.`
  }

  const needsHooks = tracked.filter((adapter) => {
    const status = connectorStatus(health, adapter.source)
    return (
      status === 'notInstalled' ||
      status === 'notFound' ||
      status === 'actionNeeded' ||
      status === 'helperMissing' ||
      status === 'hooksMisconfigured'
    )
  })
  if (needsHooks.length === tracked.length) {
    return 'No agent sessions yet. Install or repair llm_notch hooks from the dashboard Integrations tab, then start an agent session.'
  }

  return 'No agent sessions yet. Start an integration to see live sessions here.'
}

export function sessionsNeedingAttention(sessions: AgentSession[]): AgentSession[] {
  return sessions
    .filter((session) => session.attention !== 'none')
    .sort((left, right) => right.lastEventAtMs - left.lastEventAtMs)
}

export function activeSessions(sessions: AgentSession[]): AgentSession[] {
  return sessions
    .filter((session) => session.status === 'running' || session.status === 'waiting')
    .sort((left, right) => right.lastEventAtMs - left.lastEventAtMs)
}

export function recentSessions(sessions: AgentSession[]): AgentSession[] {
  return [...sessions].sort((left, right) => right.lastEventAtMs - left.lastEventAtMs)
}

export function findAdapterForSession(
  adapters: AdapterCapabilities[],
  session: AgentSession,
): AdapterCapabilities | undefined {
  return adapters.find((adapter) => adapter.source === session.source)
}

export function isNotifyOnlyAdapter(adapter: AdapterCapabilities | undefined): boolean {
  if (!adapter) {
    return true
  }
  return !adapter.decisionResponse
}

export function decisionMatchesSession(
  decision: DecisionRequest,
  session: AgentSession,
): boolean {
  return (
    decision.sessionId === session.id || decision.sessionId === session.externalSessionId
  )
}

export function findSessionForDecision(
  sessions: AgentSession[],
  decision: DecisionRequest,
): AgentSession | undefined {
  return sessions.find((session) => decisionMatchesSession(decision, session))
}
