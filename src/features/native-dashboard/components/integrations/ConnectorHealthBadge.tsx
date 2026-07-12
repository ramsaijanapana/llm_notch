import type {
  AgentSource,
  ConnectorUserStatus,
  DetectedConnector,
  ExternalTrustAction,
} from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type { IntegrationCardState } from '../../types/contracts'
import {
  connectorStatusBadgeTone,
  connectorStatusGuidance,
  effectiveConnectorStatusLabel,
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
  statusLabel?: string | undefined
  detail?: string | undefined
  detected?: DetectedConnector | undefined
  externalTrustActions?: ExternalTrustAction[] | undefined
}

export function ConnectorHealthBadge({
  source,
  status,
  statusLabel,
  detail,
  detected,
  externalTrustActions = [],
}: ConnectorHealthBadgeProps) {
  const tone = connectorStatusBadgeTone(status)
  const label = statusLabel ?? effectiveConnectorStatusLabel(status, detected)
  const guidance = detail ?? connectorStatusGuidance(source, status, externalTrustActions, detected)

  return (
    <div>
      <p className={styles.muted}>
        Health: <span className={BADGE_CLASS[tone]}>{label}</span>
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
