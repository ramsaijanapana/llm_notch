import {
  ArrowRight,
  CheckCircle2,
  Loader2,
  MessageSquare,
  RotateCcw,
  ThumbsDown,
  ThumbsUp,
} from 'lucide-react'
import { type RefObject, useEffect, useId, useRef, useState } from 'react'
import { useSimulation } from '../model/SimulationProvider'
import type { AgentSession, SessionPhase } from '../model/simulation.types'
import styles from './notchDemo.module.css'
import { getPhaseMeta } from './phaseDisplay'

function askAgentQuestionForPhase(phase: SessionPhase): string {
  switch (phase) {
    case 'needsApproval':
      return 'Why does this command need approval before you continue?'
    case 'needsAnswer':
      return 'What trade-offs should I weigh before choosing an answer?'
    case 'paused':
      return 'What blocked this session and how should I recover?'
    case 'completed':
      return 'Summarize the completed work for the release notes.'
    default:
      return 'What should I prioritize next in this workspace?'
  }
}

type DecisionPanelProps = {
  session: AgentSession
  jumpTriggerRef?: RefObject<HTMLButtonElement | null>
}

export function DecisionPanel({ session, jumpTriggerRef }: DecisionPanelProps) {
  const { dispatch } = useSimulation()
  const answerId = useId()
  const answerErrorId = useId()
  const statusRef = useRef<HTMLHeadingElement>(null)
  const localJumpRef = useRef<HTMLButtonElement>(null)
  const jumpRef = jumpTriggerRef ?? localJumpRef
  const [answer, setAnswer] = useState('')
  const [answerError, setAnswerError] = useState<string | null>(null)

  const { label, Icon, tone } = getPhaseMeta(session.phase)
  const statusClass = `status${capitalize(tone)}`

  const sessionKeyRef = useRef(`${session.id}:${session.phase}`)

  useEffect(() => {
    const nextKey = `${session.id}:${session.phase}`
    if (sessionKeyRef.current === nextKey) {
      return
    }

    sessionKeyRef.current = nextKey
    setAnswer('')
    setAnswerError(null)
  }, [session.id, session.phase])

  const focusStatus = () => {
    statusRef.current?.focus()
  }

  const handleSubmitAnswer = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const trimmed = answer.trim()
    if (!trimmed) {
      setAnswerError('Enter an answer before submitting.')
      return
    }
    setAnswerError(null)
    dispatch({ type: 'SUBMIT_ANSWER', answer: trimmed })
    setAnswer('')
    focusStatus()
  }

  const handleAskAgent = () => {
    dispatch({ type: 'ASK_AGENT', question: askAgentQuestionForPhase(session.phase) })
  }

  return (
    <section className={styles.panel} aria-label="Decision controls">
      <h3 className={styles.panelTitle}>Decision panel</h3>

      <div className={styles.decisionBody}>
        <div className={`${styles.statusBanner} ${styles[statusClass]}`}>
          <Icon
            size={18}
            aria-hidden="true"
            className={session.phase === 'running' ? styles.spin : undefined}
          />
          <div>
            <h4 ref={statusRef} tabIndex={-1} className={styles.statusSummary}>
              <strong>{label}</strong>
            </h4>
            {session.phase === 'running' && (
              <p>Agent is executing simulated work. Metrics and logs update on each tick.</p>
            )}
            {session.phase === 'completed' && (
              <p>This session finished successfully. No further action is required.</p>
            )}
            {session.phase === 'paused' && session.blockedReason && <p>{session.blockedReason}</p>}
            {session.phase === 'needsApproval' && session.prompt && <p>{session.prompt}</p>}
            {session.phase === 'needsAnswer' && session.prompt && <p>{session.prompt}</p>}
          </div>
        </div>

        {session.phase === 'needsApproval' && (
          <div className={styles.actionRow}>
            <button
              type="button"
              className={`${styles.btn} ${styles.btnPrimary}`}
              onClick={() => {
                dispatch({ type: 'APPROVE' })
                focusStatus()
              }}
            >
              <ThumbsUp size={16} aria-hidden="true" />
              Approve
            </button>
            <button
              type="button"
              className={`${styles.btn} ${styles.btnDanger}`}
              onClick={() => {
                dispatch({ type: 'REJECT' })
                focusStatus()
              }}
            >
              <ThumbsDown size={16} aria-hidden="true" />
              Reject
            </button>
          </div>
        )}

        {session.phase === 'needsAnswer' && (
          <form className={styles.formField} onSubmit={handleSubmitAnswer} noValidate>
            <label className={styles.formLabel} htmlFor={answerId}>
              Your answer
            </label>
            <input
              id={answerId}
              className={styles.textInput}
              type="text"
              value={answer}
              onChange={(event) => {
                setAnswer(event.target.value)
                if (answerError) setAnswerError(null)
              }}
              aria-invalid={answerError ? true : undefined}
              aria-describedby={answerError ? answerErrorId : undefined}
              placeholder="Type a simulated response"
            />
            {answerError ? (
              <p id={answerErrorId} className={styles.fieldError} role="alert">
                {answerError}
              </p>
            ) : null}
            <button type="submit" className={`${styles.btn} ${styles.btnPrimary}`}>
              Submit answer
            </button>
          </form>
        )}

        {session.phase === 'paused' && (
          <div className={styles.actionRow}>
            <button
              type="button"
              className={`${styles.btn} ${styles.btnPrimary}`}
              onClick={() => {
                dispatch({ type: 'RETRY' })
                focusStatus()
              }}
            >
              <RotateCcw size={16} aria-hidden="true" />
              Retry
            </button>
          </div>
        )}

        {session.phase === 'completed' && (
          <div className={`${styles.statusBanner} ${styles.statusSuccess}`}>
            <CheckCircle2 size={18} aria-hidden="true" />
            <p>Session complete. Playback can continue for other agents.</p>
          </div>
        )}

        {session.phase === 'running' && (
          <div className={`${styles.statusBanner} ${styles.statusInfo}`}>
            <Loader2 size={18} aria-hidden="true" className={styles.spin} />
            <p>Running — approve, answer, and terminal actions apply to the selected session.</p>
          </div>
        )}

        <div className={styles.actionRow}>
          <button
            type="button"
            className={styles.btn}
            onClick={handleAskAgent}
            disabled={session.phase === 'completed'}
            aria-disabled={session.phase === 'completed'}
          >
            <MessageSquare size={16} aria-hidden="true" />
            Ask agent
          </button>
          <button
            ref={jumpRef}
            type="button"
            className={styles.btn}
            onClick={() => dispatch({ type: 'JUMP' })}
          >
            <ArrowRight size={16} aria-hidden="true" />
            Jump to workspace
          </button>
        </div>

        <p className={styles.hint}>
          Ask agent uses a local prefilled prompt. Jump opens the in-frame terminal — simulation
          only, no OS access.
        </p>
      </div>
    </section>
  )
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1)
}
