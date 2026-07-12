import { useCallback, useEffect, useState } from 'react'
import type { IntegrationHealthReport, NativeClient } from '../../../native/types.ts'

export function useIntegrationHealth(client: NativeClient) {
  const [health, setHealth] = useState<IntegrationHealthReport | null>(null)

  const refreshHealth = useCallback(() => {
    void client
      .getIntegrationHealth()
      .then(setHealth)
      .catch(() => setHealth(null))
  }, [client])

  useEffect(() => {
    refreshHealth()
  }, [refreshHealth])

  useEffect(() => {
    let cancelled = false
    let subscription: { unsubscribe: () => Promise<void> } | null = null

    void client
      .subscribeConnectorHealthChanges(() => {
        if (cancelled) return
        refreshHealth()
      })
      .then((activeSubscription) => {
        if (cancelled) {
          void activeSubscription.unsubscribe()
          return
        }
        subscription = activeSubscription
      })
      .catch(() => {
        // Connector health events are optional in preview builds.
      })

    return () => {
      cancelled = true
      if (subscription) {
        void subscription.unsubscribe()
      }
    }
  }, [client, refreshHealth])

  return { health, refreshHealth }
}
