import { createInitialState, JUMP_ANNOUNCEMENT } from './scenario.ts'
import type {
  AgentSession,
  SessionEvent,
  SimulationAction,
  SimulationState,
} from './simulation.types.ts'

const ASK_AGENT_REPLY =
  'Acknowledged. I will continue with the current approach and report back in the event log.'

const ASK_AGENT_REPLIES: Record<AgentSession['phase'], string> = {
  running: ASK_AGENT_REPLY,
  needsApproval:
    'Understood. I will hold the pending command until you approve or reject it in the decision panel.',
  needsAnswer:
    'Noted. I will factor your guidance into the next step once you submit an answer above.',
  paused:
    'Copy. I will stay paused until you retry or change the blocked action in the decision panel.',
  completed: ASK_AGENT_REPLY,
}

function askAgentReplyForPhase(phase: AgentSession['phase']): string {
  return ASK_AGENT_REPLIES[phase]
}

function sessionLabel(session: AgentSession): string {
  return session.role
}

function markInteracted(state: SimulationState): SimulationState {
  if (state.hasInteracted) {
    return state
  }

  return { ...state, hasInteracted: true }
}

function getSelectedSession(state: SimulationState): AgentSession | undefined {
  return state.sessions.find((session) => session.id === state.selectedId)
}

function updateSession(
  state: SimulationState,
  sessionId: string,
  updater: (session: AgentSession) => AgentSession,
): SimulationState {
  const index = state.sessions.findIndex((session) => session.id === sessionId)
  if (index === -1) {
    return state
  }

  const sessions = state.sessions.slice()
  const current = sessions[index]
  if (!current) {
    return state
  }
  sessions[index] = updater(current)
  return { ...state, sessions }
}

function appendEvent(
  session: AgentSession,
  tick: number,
  event: Omit<SessionEvent, 'id' | 'tick' | 'timestamp'>,
): AgentSession {
  const nextEvent: SessionEvent = {
    ...event,
    id: `${session.id}-evt-${session.events.length}`,
    tick,
    timestamp: session.elapsedSeconds,
  }

  return {
    ...session,
    events: [...session.events, nextEvent],
  }
}

function sessionTickRates(sessionId: string): {
  progress: number
  tokenCount: number
  costCents: number
} {
  const seed = sessionId.split('').reduce((total, character) => total + character.charCodeAt(0), 0)

  return {
    progress: 2,
    tokenCount: 36 + (seed % 17),
    costCents: seed % 2 === 0 ? 1 : 0,
  }
}

function advanceRunningSessions(state: SimulationState): SimulationState {
  const nextTick = state.tick + 1

  const sessions = state.sessions.map((session) => {
    if (session.phase !== 'running') {
      return session
    }

    const rates = sessionTickRates(session.id)
    const nextProgress = Math.min(100, session.progress + rates.progress)
    let nextSession: AgentSession = {
      ...session,
      progress: nextProgress,
      tokenCount: session.tokenCount + rates.tokenCount,
      costCents: session.costCents + rates.costCents,
      elapsedSeconds: session.elapsedSeconds + 1,
    }

    if (nextProgress >= 100) {
      nextSession = appendEvent(nextSession, nextTick, {
        kind: 'system',
        message: 'Session marked complete.',
        actor: 'system',
      })
      nextSession = {
        ...nextSession,
        phase: 'completed',
        progress: 100,
      }
    }

    return nextSession
  })

  return {
    ...state,
    tick: nextTick,
    sessions,
  }
}

function clearOptionalPrompt(session: AgentSession): AgentSession {
  const { prompt: _prompt, blockedReason: _blockedReason, ...rest } = session
  return rest
}

export function simulationReducer(
  state: SimulationState,
  action: SimulationAction,
): SimulationState {
  switch (action.type) {
    case 'SELECT_SESSION': {
      if (!state.sessions.some((session) => session.id === action.sessionId)) {
        return state
      }

      return markInteracted({
        ...state,
        selectedId: action.sessionId,
        announcement: null,
      })
    }

    case 'SET_EXPANDED':
      return markInteracted({
        ...state,
        expanded: action.expanded,
      })

    case 'TOGGLE_EXPANDED':
      return markInteracted({
        ...state,
        expanded: !state.expanded,
      })

    case 'PLAY':
      return markInteracted({
        ...state,
        playing: true,
      })

    case 'PAUSE':
      return markInteracted({
        ...state,
        playing: false,
      })

    case 'TOGGLE_PLAYBACK':
      return markInteracted({
        ...state,
        playing: !state.playing,
      })

    case 'APPROVE': {
      const selected = getSelectedSession(state)
      if (selected?.phase !== 'needsApproval') {
        return state
      }

      return {
        ...markInteracted(
          updateSession(state, selected.id, (session) =>
            appendEvent(
              {
                ...clearOptionalPrompt(session),
                phase: 'running',
              },
              state.tick,
              {
                kind: 'approval',
                message: 'Permission approved. Resuming work.',
                actor: 'user',
              },
            ),
          ),
        ),
        announcement: `${sessionLabel(selected)} approved. Session resumed.`,
      }
    }

    case 'REJECT': {
      const selected = getSelectedSession(state)
      if (selected?.phase !== 'needsApproval') {
        return state
      }

      return {
        ...markInteracted(
          updateSession(state, selected.id, (session) =>
            appendEvent(
              {
                ...session,
                phase: 'paused',
                blockedReason: 'Permission denied by operator.',
              },
              state.tick,
              {
                kind: 'approval',
                message: 'Permission rejected. Session paused.',
                actor: 'user',
              },
            ),
          ),
        ),
        announcement: `${sessionLabel(selected)} rejected. Session paused.`,
      }
    }

    case 'SUBMIT_ANSWER': {
      const answer = action.answer.trim()
      if (!answer) {
        return state
      }

      const selected = getSelectedSession(state)
      if (selected?.phase !== 'needsAnswer') {
        return state
      }

      return {
        ...markInteracted(
          updateSession(state, selected.id, (session) =>
            appendEvent(
              {
                ...clearOptionalPrompt(session),
                phase: 'running',
              },
              state.tick,
              {
                kind: 'answer',
                message: answer,
                actor: 'user',
              },
            ),
          ),
        ),
        announcement: `${sessionLabel(selected)} answered. Session resumed.`,
      }
    }

    case 'RETRY': {
      const selected = getSelectedSession(state)
      if (selected?.phase !== 'paused') {
        return state
      }

      return {
        ...markInteracted(
          updateSession(state, selected.id, (session) =>
            appendEvent(
              {
                ...clearOptionalPrompt(session),
                phase: 'running',
              },
              state.tick,
              {
                kind: 'log',
                message: 'Retry requested. Resuming session.',
                actor: 'user',
              },
            ),
          ),
        ),
        announcement: `${sessionLabel(selected)} retry requested. Session resumed.`,
      }
    }

    case 'ASK_AGENT': {
      const question = action.question.trim()
      if (!question) {
        return state
      }

      const selected = getSelectedSession(state)
      if (!selected || selected.phase === 'completed') {
        return state
      }

      const reply = askAgentReplyForPhase(selected.phase)

      return {
        ...markInteracted(
          updateSession(state, selected.id, (session) =>
            appendEvent(
              appendEvent(session, state.tick, {
                kind: 'question',
                message: question,
                actor: 'user',
              }),
              state.tick,
              {
                kind: 'answer',
                message: reply,
                actor: 'agent',
              },
            ),
          ),
        ),
        announcement: `${sessionLabel(selected)} question sent. Agent replied in the event log.`,
      }
    }

    case 'JUMP':
      return markInteracted({
        ...state,
        terminalOpen: true,
        announcement: JUMP_ANNOUNCEMENT,
      })

    case 'CLOSE_TERMINAL':
      return markInteracted({
        ...state,
        terminalOpen: false,
      })

    case 'TICK':
      if (!state.playing) {
        return state
      }
      return advanceRunningSessions(state)

    case 'SET_REDUCED_MOTION': {
      if (state.prefersReducedMotion === action.value) {
        return state
      }

      return {
        ...state,
        prefersReducedMotion: action.value,
        playing: action.value ? false : state.playing,
      }
    }

    case 'RESET':
      return {
        ...createInitialState(state.prefersReducedMotion),
        announcement: 'Simulation reset.',
      }

    default:
      return state
  }
}

export { ASK_AGENT_REPLY }
