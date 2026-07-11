import type { AgentSession } from '../../../native/contracts'
import { getSessionDotTone, type SessionDotTone } from '../model/overlay.selectors'
import styles from './overlay.module.css'

interface SessionDotProps {
  session: AgentSession
}

function dotClass(tone: SessionDotTone): string {
  switch (tone) {
    case 'attention':
      return styles.dotAttention ?? ''
    case 'running':
      return styles.dotRunning ?? ''
    case 'waiting':
      return styles.dotWaiting ?? ''
    case 'paused':
      return styles.dotPaused ?? ''
    case 'completed':
      return styles.dotCompleted ?? ''
    case 'failed':
      return styles.dotFailed ?? ''
    case 'stale':
      return styles.dotStale ?? ''
    case 'starting':
      return styles.dotStarting ?? ''
  }
}

export function SessionDot({ session }: SessionDotProps) {
  const tone = getSessionDotTone(session)
  return (
    <span
      className={`${styles.sessionDot} ${dotClass(tone)}`}
      title={session.label}
      data-testid={`session-dot-${session.id}`}
    />
  )
}
