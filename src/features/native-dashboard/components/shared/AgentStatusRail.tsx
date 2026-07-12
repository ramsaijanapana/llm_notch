import type { LucideIcon } from 'lucide-react'
import { Gem, MousePointer2, Sparkles, Terminal } from 'lucide-react'
import type { AgentSource } from '../../../../native/contracts'
import styles from '../../styles/dashboard.module.css'
import type { AgentStatusEntry } from '../../types/contracts'
import { agentLabel } from '../../utils/formatters'
import { connectorStatusBadgeTone, connectorStatusLabel } from '../../utils/integrationLabels'

const AGENT_ICONS: Partial<Record<AgentSource, LucideIcon>> = {
  cursor: MousePointer2,
  claudeCode: Sparkles,
  codex: Terminal,
  gemini: Gem,
}

const STATUS_DOT_CLASS = {
  info: styles.statusDotInfo,
  warning: styles.statusDotWarning,
  error: styles.statusDotError,
  success: styles.statusDotSuccess,
} as const

type AgentStatusRailProps = {
  agents: AgentStatusEntry[]
}

export function AgentStatusRail({ agents }: AgentStatusRailProps) {
  if (agents.length === 0) {
    return null
  }

  return (
    <section className={styles.agentRail} aria-label="Agent status">
      <ul className={styles.agentRailList}>
        {agents.map((entry) => {
          const Icon = AGENT_ICONS[entry.source]
          const tone = connectorStatusBadgeTone(entry.status)
          const statusLabel = connectorStatusLabel(entry.status)
          const activity =
            entry.attentionSessions && entry.attentionSessions > 0
              ? `${entry.attentionSessions} need attention`
              : entry.activeSessions && entry.activeSessions > 0
                ? `${entry.activeSessions} active`
                : 'Idle'

          return (
            <li key={entry.source}>
              <article
                className={styles.agentRailCard}
                aria-label={`${agentLabel(entry.source)}: ${statusLabel}`}
              >
                <span className={styles.agentRailIcon} aria-hidden="true">
                  {Icon ? <Icon size={14} strokeWidth={2} /> : null}
                </span>
                <span className={styles.agentRailBody}>
                  <span className={styles.agentRailName}>{agentLabel(entry.source)}</span>
                  <span className={styles.agentRailMeta}>
                    <span
                      className={`${styles.statusDot} ${STATUS_DOT_CLASS[tone]}`}
                      aria-hidden="true"
                    />
                    <span>{statusLabel}</span>
                    <span className={styles.agentRailDivider} aria-hidden="true">
                      ·
                    </span>
                    <span>{activity}</span>
                  </span>
                </span>
              </article>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
