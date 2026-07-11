import { describe, expect, it } from 'vitest'
import { createInitialState, JUMP_ANNOUNCEMENT } from './scenario.ts'
import { ASK_AGENT_REPLY, simulationReducer } from './simulation.reducer.ts'
import type { SimulationState } from './simulation.types.ts'

function reduce(state: SimulationState, ...actions: Parameters<typeof simulationReducer>[1][]) {
  return actions.reduce((nextState, action) => simulationReducer(nextState, action), state)
}

function sessionById(state: SimulationState, id: string) {
  const session = state.sessions.find((entry) => entry.id === id)
  if (!session) {
    throw new Error(`Missing session ${id}`)
  }
  return session
}

describe('createInitialState', () => {
  it('creates the four scripted sessions with expected phases', () => {
    const state = createInitialState()

    expect(state.sessions.map((session) => [session.role, session.task, session.phase])).toEqual([
      ['Builder', 'Refactor authentication flow', 'running'],
      ['Tester', 'Run regression suite', 'needsApproval'],
      ['Reviewer', 'Choose API error shape', 'needsAnswer'],
      ['Writer', 'Update release notes', 'completed'],
    ])
  })

  it('includes the pending approval command on the tester session', () => {
    const tester = createInitialState().sessions.find((session) => session.id === 'tester')
    expect(tester?.prompt).toBe('Approve running: npm test --coverage')
  })

  it('starts playback when reduced motion is not preferred', () => {
    expect(createInitialState(false).playing).toBe(true)
    expect(createInitialState().playing).toBe(true)
  })

  it('starts paused when reduced motion is preferred', () => {
    expect(createInitialState(true).playing).toBe(false)
    expect(createInitialState(true).prefersReducedMotion).toBe(true)
  })
})

describe('simulationReducer transitions', () => {
  it('selects sessions and marks interaction', () => {
    const initial = createInitialState()
    const next = simulationReducer(initial, { type: 'SELECT_SESSION', sessionId: 'tester' })

    expect(next.selectedId).toBe('tester')
    expect(next.hasInteracted).toBe(true)
  })

  it('ignores invalid session selection', () => {
    const initial = createInitialState()
    const next = simulationReducer(initial, { type: 'SELECT_SESSION', sessionId: 'missing' })

    expect(next).toBe(initial)
  })

  it('toggles and sets expanded state', () => {
    const initial = createInitialState()
    const expanded = simulationReducer(initial, { type: 'SET_EXPANDED', expanded: true })
    const collapsed = simulationReducer(expanded, { type: 'SET_EXPANDED', expanded: false })
    const toggled = simulationReducer(initial, { type: 'TOGGLE_EXPANDED' })

    expect(expanded.expanded).toBe(true)
    expect(collapsed.expanded).toBe(false)
    expect(toggled.expanded).toBe(true)
  })

  it('controls playback state', () => {
    const initial = createInitialState(true)
    const playing = simulationReducer(initial, { type: 'PLAY' })
    const paused = simulationReducer(playing, { type: 'PAUSE' })
    const toggled = simulationReducer(initial, { type: 'TOGGLE_PLAYBACK' })

    expect(playing.playing).toBe(true)
    expect(paused.playing).toBe(false)
    expect(toggled.playing).toBe(true)
  })

  it('approves the selected approval session', () => {
    const initial = reduce(createInitialState(), {
      type: 'SELECT_SESSION',
      sessionId: 'tester',
    })
    const next = simulationReducer(initial, { type: 'APPROVE' })
    const tester = sessionById(next, 'tester')

    expect(tester.phase).toBe('running')
    expect(tester.events.at(-1)?.message).toBe('Permission approved. Resuming work.')
  })

  it('rejects the selected approval session into paused with a reason', () => {
    const initial = reduce(createInitialState(), {
      type: 'SELECT_SESSION',
      sessionId: 'tester',
    })
    const next = simulationReducer(initial, { type: 'REJECT' })
    const tester = sessionById(next, 'tester')

    expect(tester.phase).toBe('paused')
    expect(tester.blockedReason).toBe('Permission denied by operator.')
  })

  it('submits trimmed answers and resumes the selected question session', () => {
    const initial = reduce(createInitialState(), {
      type: 'SELECT_SESSION',
      sessionId: 'reviewer',
    })
    const next = simulationReducer(initial, {
      type: 'SUBMIT_ANSWER',
      answer: '  { message: string }  ',
    })
    const reviewer = sessionById(next, 'reviewer')

    expect(reviewer.phase).toBe('running')
    expect(reviewer.prompt).toBeUndefined()
    expect(reviewer.events.at(-1)).toMatchObject({
      kind: 'answer',
      message: '{ message: string }',
      actor: 'user',
    })
  })

  it('retries paused sessions back to running', () => {
    const paused = reduce(
      createInitialState(),
      { type: 'SELECT_SESSION', sessionId: 'tester' },
      { type: 'REJECT' },
    )
    const next = simulationReducer(paused, { type: 'RETRY' })
    const tester = sessionById(next, 'tester')

    expect(tester.phase).toBe('running')
    expect(tester.blockedReason).toBeUndefined()
  })

  it('appends ask-agent exchanges without changing phase', () => {
    const initial = createInitialState()
    const next = simulationReducer(initial, {
      type: 'ASK_AGENT',
      question: 'Can you summarize the auth changes?',
    })
    const builder = sessionById(next, 'builder')

    expect(builder.phase).toBe('running')
    expect(builder.events.at(-2)).toMatchObject({
      kind: 'question',
      message: 'Can you summarize the auth changes?',
      actor: 'user',
    })
    expect(builder.events.at(-1)).toMatchObject({
      kind: 'answer',
      message: ASK_AGENT_REPLY,
      actor: 'agent',
    })
  })

  it('opens the terminal drawer and announces jump requests', () => {
    const next = simulationReducer(createInitialState(), { type: 'JUMP' })

    expect(next.terminalOpen).toBe(true)
    expect(next.announcement).toBe(JUMP_ANNOUNCEMENT)
  })

  it('closes the terminal drawer', () => {
    const open = simulationReducer(createInitialState(), { type: 'JUMP' })
    const next = simulationReducer(open, { type: 'CLOSE_TERMINAL' })

    expect(next.terminalOpen).toBe(false)
  })
})

describe('simulationReducer guards', () => {
  it('ignores approve, reject, answer, and retry on invalid phases', () => {
    const initial = createInitialState()

    expect(simulationReducer(initial, { type: 'APPROVE' })).toBe(initial)
    expect(simulationReducer(initial, { type: 'REJECT' })).toBe(initial)
    expect(simulationReducer(initial, { type: 'SUBMIT_ANSWER', answer: 'yes' })).toBe(initial)
    expect(simulationReducer(initial, { type: 'RETRY' })).toBe(initial)
  })

  it('ignores blank answers', () => {
    const initial = reduce(createInitialState(), {
      type: 'SELECT_SESSION',
      sessionId: 'reviewer',
    })

    expect(simulationReducer(initial, { type: 'SUBMIT_ANSWER', answer: '   ' })).toBe(initial)
  })

  it('ignores blank ask-agent prompts', () => {
    const initial = createInitialState()
    expect(simulationReducer(initial, { type: 'ASK_AGENT', question: '  ' })).toBe(initial)
  })

  it('ignores ask-agent prompts for completed sessions', () => {
    const initial = reduce(createInitialState(), {
      type: 'SELECT_SESSION',
      sessionId: 'writer',
    })

    expect(simulationReducer(initial, { type: 'ASK_AGENT', question: 'Any follow-up?' })).toBe(
      initial,
    )
  })
})

describe('deterministic TICK behavior', () => {
  it('advances only running sessions and completes at 100', () => {
    let state = reduce(createInitialState(true), { type: 'PLAY' })
    const snapshots: Array<{ tick: number; progress: number; phase: string }> = []

    for (let index = 0; index < 40; index += 1) {
      state = simulationReducer(state, { type: 'TICK' })
      snapshots.push({
        tick: state.tick,
        progress: sessionById(state, 'builder').progress,
        phase: sessionById(state, 'builder').phase,
      })
    }

    expect(snapshots[0]).toEqual({ tick: 1, progress: 40, phase: 'running' })
    expect(sessionById(state, 'tester').phase).toBe('needsApproval')
    expect(sessionById(state, 'reviewer').phase).toBe('needsAnswer')
    expect(sessionById(state, 'writer').phase).toBe('completed')

    const completionIndex = snapshots.findIndex((entry) => entry.phase === 'completed')
    expect(completionIndex).toBe(30)
    expect(snapshots[completionIndex]?.progress).toBe(100)
  })

  it('uses deterministic usage increments per session id', () => {
    const first = reduce(createInitialState(true), { type: 'PLAY' }, { type: 'TICK' })
    const second = reduce(createInitialState(true), { type: 'PLAY' }, { type: 'TICK' })
    const builderFirst = sessionById(first, 'builder')
    const builderSecond = sessionById(second, 'builder')

    expect(builderFirst.tokenCount).toBe(builderSecond.tokenCount)
    expect(builderFirst.costCents).toBe(builderSecond.costCents)
    expect(builderFirst.elapsedSeconds).toBe(143)
  })
})

describe('RESET', () => {
  it('restores the exact initial scenario while preserving reduced motion', () => {
    const mutated = reduce(
      createInitialState(true),
      { type: 'SELECT_SESSION', sessionId: 'writer' },
      { type: 'SET_EXPANDED', expanded: true },
      { type: 'JUMP' },
      { type: 'PAUSE' },
      { type: 'TICK' },
      { type: 'TICK' },
    )

    const reset = simulationReducer(mutated, { type: 'RESET' })

    expect(reset).toEqual({
      ...createInitialState(true),
      announcement: 'Simulation reset.',
    })
    expect(reset.playing).toBe(false)
  })

  it('keeps autoplay after reset when reduced motion is not preferred', () => {
    const reset = simulationReducer(createInitialState(false), { type: 'RESET' })

    expect(reset.playing).toBe(true)
    expect(reset.prefersReducedMotion).toBe(false)
    expect(reset.announcement).toBe('Simulation reset.')
  })
})

describe('TICK while paused', () => {
  it('does not advance sessions when playback is paused', () => {
    const initial = createInitialState(true)
    const next = simulationReducer(initial, { type: 'TICK' })

    expect(next).toBe(initial)
    expect(next.tick).toBe(0)
  })
})

describe('SET_REDUCED_MOTION', () => {
  it('pauses playback when reduced motion becomes preferred', () => {
    const initial = createInitialState(false)
    const next = simulationReducer(initial, { type: 'SET_REDUCED_MOTION', value: true })

    expect(next.prefersReducedMotion).toBe(true)
    expect(next.playing).toBe(false)
  })
})

describe('action announcements', () => {
  it('announces approve, reject, answer, retry, ask, and reset outcomes', () => {
    const approvalState = reduce(createInitialState(true), {
      type: 'SELECT_SESSION',
      sessionId: 'tester',
    })

    expect(simulationReducer(approvalState, { type: 'APPROVE' }).announcement).toBe(
      'Tester approved. Session resumed.',
    )

    expect(simulationReducer(approvalState, { type: 'REJECT' }).announcement).toBe(
      'Tester rejected. Session paused.',
    )

    const answerState = reduce(createInitialState(true), {
      type: 'SELECT_SESSION',
      sessionId: 'reviewer',
    })

    expect(
      simulationReducer(answerState, { type: 'SUBMIT_ANSWER', answer: 'RFC 7807' }).announcement,
    ).toBe('Reviewer answered. Session resumed.')

    const pausedState = reduce(
      createInitialState(true),
      { type: 'SELECT_SESSION', sessionId: 'tester' },
      { type: 'REJECT' },
    )

    expect(simulationReducer(pausedState, { type: 'RETRY' }).announcement).toBe(
      'Tester retry requested. Session resumed.',
    )

    expect(
      simulationReducer(createInitialState(true), {
        type: 'ASK_AGENT',
        question: 'Status?',
      }).announcement,
    ).toBe('Builder question sent. Agent replied in the event log.')

    expect(simulationReducer(createInitialState(), { type: 'RESET' }).announcement).toBe(
      'Simulation reset.',
    )
  })
})
