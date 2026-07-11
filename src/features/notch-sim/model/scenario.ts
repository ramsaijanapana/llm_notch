import type { AgentSession, SimulationState } from './simulation.types.ts'

const JUMP_ANNOUNCEMENT = 'Demo only: terminal focus requested.'

const INITIAL_SESSIONS: readonly AgentSession[] = [
  {
    id: 'builder',
    role: 'Builder',
    task: 'Refactor authentication flow',
    workspace: '~/projects/auth-refactor',
    phase: 'running',
    progress: 38,
    tokenCount: 12_480,
    costCents: 18,
    elapsedSeconds: 142,
    events: [
      {
        id: 'builder-evt-0',
        tick: 0,
        timestamp: 128,
        kind: 'log',
        message: 'Scanning auth middleware and session handlers.',
        actor: 'agent',
      },
      {
        id: 'builder-evt-1',
        tick: 0,
        timestamp: 136,
        kind: 'log',
        message: 'Drafting token refresh path with stricter validation.',
        actor: 'agent',
      },
    ],
  },
  {
    id: 'tester',
    role: 'Tester',
    task: 'Run regression suite',
    workspace: '~/projects/auth-refactor',
    phase: 'needsApproval',
    progress: 62,
    tokenCount: 8_920,
    costCents: 11,
    elapsedSeconds: 96,
    prompt: 'Approve running: npm test --coverage',
    events: [
      {
        id: 'tester-evt-0',
        tick: 0,
        timestamp: 84,
        kind: 'log',
        message: 'Prepared coverage run for auth middleware changes.',
        actor: 'agent',
      },
      {
        id: 'tester-evt-1',
        tick: 0,
        timestamp: 96,
        kind: 'approval',
        message: 'Requesting permission to run npm test --coverage.',
        actor: 'agent',
      },
    ],
  },
  {
    id: 'reviewer',
    role: 'Reviewer',
    task: 'Choose API error shape',
    workspace: '~/projects/auth-refactor',
    phase: 'needsAnswer',
    progress: 44,
    tokenCount: 5_640,
    costCents: 7,
    elapsedSeconds: 71,
    prompt: 'Should API errors return { error: string } or { message: string }?',
    events: [
      {
        id: 'reviewer-evt-0',
        tick: 0,
        timestamp: 58,
        kind: 'log',
        message: 'Reviewing error payload consistency across handlers.',
        actor: 'agent',
      },
      {
        id: 'reviewer-evt-1',
        tick: 0,
        timestamp: 71,
        kind: 'question',
        message: 'Should API errors return { error: string } or { message: string }?',
        actor: 'agent',
      },
    ],
  },
  {
    id: 'writer',
    role: 'Writer',
    task: 'Update release notes',
    workspace: '~/projects/auth-refactor',
    phase: 'completed',
    progress: 100,
    tokenCount: 3_210,
    costCents: 4,
    elapsedSeconds: 48,
    events: [
      {
        id: 'writer-evt-0',
        tick: 0,
        timestamp: 36,
        kind: 'log',
        message: 'Summarized auth refactor changes for the release draft.',
        actor: 'agent',
      },
      {
        id: 'writer-evt-1',
        tick: 0,
        timestamp: 48,
        kind: 'system',
        message: 'Session marked complete.',
        actor: 'system',
      },
    ],
  },
] as const

function cloneSession(session: AgentSession): AgentSession {
  const cloned: AgentSession = {
    ...session,
    events: session.events.map((event) => ({ ...event })),
  }

  if (session.prompt !== undefined) {
    cloned.prompt = session.prompt
  }

  if (session.blockedReason !== undefined) {
    cloned.blockedReason = session.blockedReason
  }

  return cloned
}

export function createInitialState(prefersReducedMotion = false): SimulationState {
  return {
    sessions: INITIAL_SESSIONS.map(cloneSession),
    selectedId: 'builder',
    expanded: false,
    playing: !prefersReducedMotion,
    prefersReducedMotion,
    terminalOpen: false,
    tick: 0,
    announcement: null,
    hasInteracted: false,
  }
}

export { INITIAL_SESSIONS, JUMP_ANNOUNCEMENT }
