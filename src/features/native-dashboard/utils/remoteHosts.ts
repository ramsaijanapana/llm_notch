import type { RemoteConnectionStatusView, RemoteHostView } from '../../../native/contracts'

export function applyRemoteConnectionStatus(
  hosts: RemoteHostView[],
  status: RemoteConnectionStatusView,
  nowMs = Date.now(),
): RemoteHostView[] {
  const index = hosts.findIndex((host) => host.config.id === status.hostId)
  if (index < 0) {
    return hosts
  }

  const host = hosts[index]!
  const nextHost: RemoteHostView = {
    ...host,
    availability: status.availability,
    connectionState: status.connectionState,
    message: status.message ?? null,
    ...(status.connectionState === 'streaming' ? { lastConnectedAtMs: nowMs } : {}),
  }

  return [...hosts.slice(0, index), nextHost, ...hosts.slice(index + 1)]
}
