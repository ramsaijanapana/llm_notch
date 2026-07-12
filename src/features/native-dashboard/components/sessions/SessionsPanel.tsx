import styles from '../../styles/dashboard.module.css'
import type { SessionsPanelProps } from '../../types/contracts'
import {
  activeSessions,
  recentSessions,
  sessionsNeedingAttention,
} from '../../utils/sessionHelpers'
import { EmptyState } from '../shared/EmptyState'
import { LoadingState } from '../shared/LoadingState'
import { AttentionQueue } from './AttentionQueue'
import { SessionDetail } from './SessionDetail'
import { SessionEventStream } from './SessionEventStream'
import { SessionList } from './SessionList'

export function SessionsPanel({
  sessions,
  selectedSessionId,
  events,
  adapters,
  pendingDecision,
  decisionRecord,
  onSelectSession,
  onOpenContext,
  onDecisionAllow,
  onDecisionDeny,
  onDecisionAnswer,
  onAcknowledge,
  loadState = 'ready',
  emptyMessage = 'No agent sessions yet. Start an integration to see live sessions here.',
}: SessionsPanelProps) {
  if (loadState === 'loading') {
    return <LoadingState label="Loading sessions…" />
  }

  if (loadState === 'empty' || sessions.length === 0) {
    return <EmptyState title="No sessions" description={emptyMessage} />
  }

  const attention = sessionsNeedingAttention(sessions)
  const active = activeSessions(sessions)
  const recent = recentSessions(sessions)
  const selected = sessions.find((session) => session.id === selectedSessionId)
  const sessionEvents = events.filter((event) => event.sessionId === selectedSessionId)

  return (
    <div className={styles.panelGridTwo}>
      <div className={styles.panelGrid}>
        <AttentionQueue
          sessions={attention}
          selectedSessionId={selectedSessionId}
          onSelectSession={onSelectSession}
          onAcknowledge={onAcknowledge}
        />
        <SessionList
          title="Active sessions"
          sessions={active}
          selectedSessionId={selectedSessionId}
          onSelectSession={onSelectSession}
          emptyLabel="No active sessions."
        />
        <SessionList
          title="Recent sessions"
          sessions={recent}
          selectedSessionId={selectedSessionId}
          onSelectSession={onSelectSession}
          emptyLabel="No recent sessions."
        />
      </div>

      <div className={styles.panelGrid}>
        <SessionDetail
          session={selected}
          adapters={adapters}
          pendingDecision={pendingDecision}
          decisionRecord={decisionRecord}
          onOpenContext={onOpenContext}
          onDecisionAllow={onDecisionAllow}
          onDecisionDeny={onDecisionDeny}
          onDecisionAnswer={onDecisionAnswer}
        />
        <SessionEventStream events={sessionEvents} />
      </div>
    </div>
  )
}
