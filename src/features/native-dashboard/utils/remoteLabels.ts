import type { RemoteConnectionState } from '../../../native/contracts'

export type RemoteConnectionBadgeTone = 'info' | 'warning' | 'error' | 'success'

export function remoteConnectionStateLabel(state: RemoteConnectionState): string {
  if (typeof state === 'string') {
    switch (state) {
      case 'disconnected':
        return 'Disconnected'
      case 'connecting':
        return 'Connecting'
      case 'authenticating':
        return 'Authenticating'
      case 'streaming':
        return 'Streaming'
      case 'failed':
        return 'Failed'
      default:
        return 'Unknown'
    }
  }

  return `Backoff (attempt ${state.backoff.attempt})`
}

export function remoteConnectionBadgeTone(state: RemoteConnectionState): RemoteConnectionBadgeTone {
  if (typeof state === 'string') {
    switch (state) {
      case 'streaming':
        return 'success'
      case 'connecting':
      case 'authenticating':
        return 'warning'
      case 'failed':
        return 'error'
      case 'disconnected':
      default:
        return 'info'
    }
  }

  return 'warning'
}

export function remoteDeploymentStepLabel(
  step: import('../../../native/contracts').RemoteDeploymentStep,
): string {
  switch (step.type) {
    case 'probeTarget':
      return 'Probe remote target'
    case 'createPrivateDirectory':
      return `Create directory ${step.remoteDirectory}`
    case 'uploadTemporary':
      return `Upload relay to ${step.remotePath}`
    case 'verifySha256':
      return 'Verify relay SHA-256'
    case 'activateAtomically':
      return `Activate relay at ${step.remotePath}`
    case 'startStdioRelay':
      return `Start stdio relay at ${step.remotePath} (event spool ${step.eventSpoolDir}; set LLM_NOTCH_EVENT_SPOOL=1 on remote hooks)`
    default:
      return 'Deployment step'
  }
}

export function remoteBackendGuidance(
  availability: 'available' | 'unavailable',
  message?: string | null,
): string | undefined {
  if (availability === 'available') {
    return undefined
  }

  return (
    message ??
    'SSH relay management is not available in this build. Host listing and lifecycle actions remain disabled until the relay backend ships.'
  )
}
