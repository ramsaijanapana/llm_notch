import type {
  AgentSource,
  ConnectorUserStatus,
  DecisionDeliveryState,
  DetectedConnector,
  ExternalTrustAction,
} from '../../../native/contracts'

export const DOCUMENTED_CONNECTOR_PATHS: Array<{
  source: AgentSource
  userPath: string
  projectPath: string
}> = [
  { source: 'cursor', userPath: '~/.cursor/hooks.json', projectPath: '<repo>/.cursor/hooks.json' },
  {
    source: 'claudeCode',
    userPath: '~/.claude/settings.json',
    projectPath: '<repo>/.claude/settings.json',
  },
  { source: 'codex', userPath: '~/.codex/hooks.json', projectPath: '<repo>/.codex/hooks.json' },
  {
    source: 'gemini',
    userPath: '~/.gemini/settings.json',
    projectPath: '<repo>/.gemini/settings.json',
  },
  {
    source: 'qwen',
    userPath: '~/.qwen/settings.json',
    projectPath: '<repo>/.qwen/settings.json',
  },
  {
    source: 'antigravityCli',
    userPath: '~/.gemini/antigravity-cli/hooks.json',
    projectPath: '<repo>/.agents/hooks.json',
  },
  {
    source: 'copilotCli',
    userPath: '~/.copilot/hooks/llm-notch.json',
    projectPath: '<repo>/.github/hooks/llm-notch.json',
  },
]

export function bestDetectedConnector(
  detected: DetectedConnector[],
  source: AgentSource,
): DetectedConnector | undefined {
  return detected
    .filter((entry) => entry.source === source)
    .sort(
      (left, right) =>
        Number(right.configPresent) - Number(left.configPresent) ||
        Number(right.executablePresent) - Number(left.executablePresent) ||
        Number(right.processRunning) - Number(left.processRunning),
    )[0]
}

export function isDetectedConnectorVisible(entry: DetectedConnector): boolean {
  return entry.configPresent || entry.executablePresent || Boolean(entry.processRunning)
}

export function detectedConnectorSummary(entry: DetectedConnector): string {
  if (entry.managedEntriesPresent) {
    return 'Hooks connected'
  }
  if (entry.configPresent) {
    return 'Hook config present — hooks need repair'
  }
  if (entry.executablePresent) {
    return entry.executablePath
      ? `CLI installed (${entry.executablePath}) — hooks missing`
      : 'CLI installed — hooks missing'
  }
  if (entry.processRunning) {
    return entry.runningProcessName
      ? `Process running (${entry.runningProcessName}) — session not verified`
      : 'Process running — session not verified'
  }
  return 'Not detected'
}

export interface ConnectorInstallationLayers {
  cli: string
  hookConfig: string
  managedHooks: string
  process: string
  traffic: string
}

export function connectorInstallationLayers(
  detected: DetectedConnector | undefined,
  status: ConnectorUserStatus,
  lastEventAtMs?: number,
): ConnectorInstallationLayers {
  const cli = detected?.executablePresent
    ? detected.executablePath
      ? `Installed (${detected.executablePath})`
      : 'Installed'
    : 'Not found'

  const hookConfig = detected?.configPresent
    ? `Present (${detected.displayPath})`
    : 'Not found'

  let managedHooks = 'Not installed'
  if (detected?.managedEntriesPresent) {
    managedHooks = 'Managed by llm-notch'
  } else if (detected?.configPresent) {
    managedHooks = 'Needs repair'
  } else if (detected?.executablePresent) {
    managedHooks = 'Missing — use Connect'
  }

  const process = detected?.processRunning
    ? detected.runningProcessName
      ? `Running (${detected.runningProcessName})`
      : 'Running'
    : 'Not observed'

  let traffic = 'No events yet'
  if (status === 'connected') {
    traffic = lastEventAtMs ? 'Connected' : 'Connected'
  } else if (status === 'waitingFirstEvent') {
    traffic = 'Waiting for first event'
  } else if (lastEventAtMs) {
    traffic = 'Recent events observed'
  }

  return { cli, hookConfig, managedHooks, process, traffic }
}

export function effectiveConnectorStatusLabel(
  status: ConnectorUserStatus,
  detected?: DetectedConnector,
): string {
  if (detected?.managedEntriesPresent) {
    return connectorStatusLabel(status)
  }
  if (detected?.configPresent && !detected.managedEntriesPresent) {
    return 'Hooks need repair'
  }
  if (detected?.executablePresent && !detected.configPresent) {
    return 'CLI installed — hooks missing'
  }
  if (detected?.processRunning && !detected.configPresent && !detected.executablePresent) {
    return 'Process running — session not verified'
  }
  return connectorStatusLabel(status)
}

const STATUS_LABELS: Record<ConnectorUserStatus, string> = {
  notFound: 'Not found',
  notInstalled: 'Not installed',
  actionNeeded: 'Action needed',
  waitingFirstEvent: 'Waiting for first event',
  helperMissing: 'Helper missing',
  hooksMisconfigured: 'Hooks misconfigured',
  connected: 'Connected',
  driftDetected: 'Drift detected',
  error: 'Error',
}

const STATUS_BADGE: Record<ConnectorUserStatus, 'info' | 'warning' | 'error' | 'success'> = {
  notFound: 'info',
  notInstalled: 'info',
  actionNeeded: 'warning',
  waitingFirstEvent: 'warning',
  helperMissing: 'error',
  hooksMisconfigured: 'error',
  connected: 'success',
  driftDetected: 'warning',
  error: 'error',
}

const DELIVERY_LABELS: Record<DecisionDeliveryState, string> = {
  pending: 'Response pending delivery',
  delivered: 'Response delivered to agent',
  effectObserved: 'Agent acknowledged the response',
  expired: 'Request expired before delivery',
  failed: 'Delivery failed — agent may continue fail-open',
}

export function connectorStatusLabel(status: ConnectorUserStatus): string {
  return STATUS_LABELS[status]
}

export function connectorStatusBadgeTone(
  status: ConnectorUserStatus,
): 'info' | 'warning' | 'error' | 'success' {
  return STATUS_BADGE[status]
}

export function connectorStatusGuidance(
  source: AgentSource,
  status: ConnectorUserStatus,
  externalTrustActions: ExternalTrustAction[] = [],
  detected?: DetectedConnector,
): string | undefined {
  if (detected?.configPresent && !detected.managedEntriesPresent) {
    return 'Hook config exists but llm_notch entries are missing. Use Repair to reconcile managed hooks.'
  }
  if (detected?.executablePresent && !detected.configPresent && status === 'notInstalled') {
    return 'Agent CLI is installed. Use Connect to wire llm_notch hooks to the bundled helper.'
  }
  if (detected?.processRunning && !detected.configPresent && !detected.executablePresent) {
    return 'An agent process is running, but hooks have not verified a session yet.'
  }
  if (source === 'cursor' && (status === 'notInstalled' || status === 'actionNeeded')) {
    return 'Enable hooks in Cursor Settings → Hooks, then restart Cursor.'
  }
  if (source === 'codex' && status === 'actionNeeded') {
    const codex = externalTrustActions.find((action) => action.kind === 'codexHooksReview')
    return (
      codex?.instructions ??
      'Open the Codex CLI, run /hooks, review each llm_notch hook definition, and trust it.'
    )
  }
  if (status === 'waitingFirstEvent') {
    return 'Integration is installed — start or resume an agent session to verify traffic.'
  }
  if (status === 'helperMissing') {
    return 'Hook commands point to a helper binary that is missing on disk. Use Repair to rewrite hooks to the bundled helper.'
  }
  if (status === 'hooksMisconfigured') {
    return 'Hook commands point to a different helper path than the bundled binary. Use Repair to reconcile paths.'
  }
  if (status === 'driftDetected') {
    return 'Configuration changed since install. Use Repair to reconcile llm_notch entries.'
  }
  if (status === 'notInstalled') {
    return 'Agent CLI or llm_notch hooks are missing. Use Connect to wire llm_notch hooks to the installed helper.'
  }
  if (status === 'notFound') {
    return 'No agent CLI or configuration file found at the documented path for this scope.'
  }
  if (status === 'error') {
    return 'Health probe failed. Try Repair or check the helper is available.'
  }
  return undefined
}

export function decisionDeliveryLabel(state: DecisionDeliveryState): string {
  return DELIVERY_LABELS[state]
}

export function applyProgressLabel(
  phase: 'applying' | 'backingUp' | 'writing' | 'verifying' | 'done' | 'failed',
): string {
  switch (phase) {
    case 'applying':
      return 'Applying…'
    case 'backingUp':
      return 'Backed up'
    case 'writing':
      return 'Written'
    case 'verifying':
      return 'Verified'
    case 'done':
      return 'Complete'
    case 'failed':
      return 'Failed'
  }
}
