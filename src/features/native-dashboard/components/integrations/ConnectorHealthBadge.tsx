import type {
  AgentSource,
  ConnectorUserStatus,
  ExternalTrustAction,
} from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type { IntegrationCardState } from '../../types/contracts'
import {
  connectorStatusBadgeTone,
  connectorStatusGuidance,
  connectorStatusLabel,
} from '../../utils/integrationLabels'

const BADGE_CLASS = {
  info: styles.badgeInfo,
  warning: styles.badgeWarning,
  error: styles.badgeError,
  success: styles.badgeSuccess,
} as const

type ConnectorHealthBadgeProps = {
  source: AgentSource
  status: ConnectorUserStatus
  detail?: string | undefined
  externalTrustActions?: ExternalTrustAction[] | undefined
}

export function ConnectorHealthBadge({
  source,
  status,
  detail,
  externalTrustActions = [],
}: ConnectorHealthBadgeProps) {
  const tone = connectorStatusBadgeTone(status)
  const guidance = detail ?? connectorStatusGuidance(source, status, externalTrustActions)

  return (
    <div>
      <p className={styles.muted}>
        Health: <span className={BADGE_CLASS[tone]}>{connectorStatusLabel(status)}</span>
      </p>
      {guidance ? (
        <p className={styles.caveat} role="status">
          {guidance}
        </p>
      ) : null}
    </div>
  )
}

export function integrationStatusFromCard(card: IntegrationCardState): ConnectorUserStatus {
  return card.status
}
