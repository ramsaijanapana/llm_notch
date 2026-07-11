import type {
  AgentSource,
  ConnectorUserStatus,
  DecisionDeliveryState,
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
]

const STATUS_LABELS: Record<ConnectorUserStatus, string> = {
  notFound: 'Not found',
  notInstalled: 'Not installed',
  actionNeeded: 'Action needed',
  waitingFirstEvent: 'Waiting for first event',
  connected: 'Connected',
  driftDetected: 'Drift detected',
  error: 'Error',
}

const STATUS_BADGE: Record<ConnectorUserStatus, 'info' | 'warning' | 'error' | 'success'> = {
  notFound: 'info',
  notInstalled: 'info',
  actionNeeded: 'warning',
  waitingFirstEvent: 'warning',
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
): string | undefined {
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
  if (status === 'driftDetected') {
    return 'Configuration changed since install. Use Repair to reconcile llm_notch entries.'
  }
  if (status === 'notFound') {
    return 'No configuration file found at the documented path for this scope.'
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
