import type { AgentSession, SimulationAction, SimulationState } from '../model/simulation.types'

export const mockSessions: AgentSession[] = [
  {
    id: 'builder',
    role: 'Builder',
    task: 'Refactor authentication flow',
    workspace: 'auth-service',
    phase: 'running',
    progress: 42,
    tokenCount: 1200,
    costCents: 18,
    elapsedSeconds: 95,
    events: [
      {
        id: 'e1',
        tick: 1,
        timestamp: 65,
        kind: 'log',
        message: 'Opened login module',
        actor: 'agent',
      },
    ],
  },
  {
    id: 'tester',
    role: 'Tester',
    task: 'Run regression suite',
    workspace: 'qa-harness',
    phase: 'needsApproval',
    progress: 60,
    tokenCount: 800,
    costCents: 12,
    elapsedSeconds: 70,
    events: [],
    prompt: 'Approve running: npm test --coverage',
  },
  {
    id: 'reviewer',
    role: 'Reviewer',
    task: 'Choose API error shape',
    workspace: 'api-gateway',
    phase: 'needsAnswer',
    progress: 25,
    tokenCount: 400,
    costCents: 6,
    elapsedSeconds: 40,
    events: [],
    prompt: 'Prefer RFC 7807 problem details or legacy envelope?',
  },
  {
    id: 'writer',
    role: 'Writer',
    task: 'Update release notes',
    workspace: 'docs',
    phase: 'completed',
    progress: 100,
    tokenCount: 300,
    costCents: 4,
    elapsedSeconds: 120,
    events: [],
  },
]

export function createMockState(overrides: Partial<SimulationState> = {}): SimulationState {
  return {
    sessions: mockSessions,
    selectedId: 'builder',
    expanded: true,
    playing: false,
    prefersReducedMotion: false,
    terminalOpen: false,
    tick: 0,
    announcement: null,
    hasInteracted: false,
    ...overrides,
  }
}

export function createMockDispatch() {
  const calls: SimulationAction[] = []
  const dispatch = (action: SimulationAction) => {
    calls.push(action)
  }
  return { dispatch, calls }
}
