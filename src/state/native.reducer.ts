import type {
  AgentSession,
  AppSnapshot,
  PublicSettings,
  SessionEvent,
} from '../native/contracts.ts'
import { PROTOCOL_VERSION } from '../native/contracts.ts'
import { mergeMetricsFrame } from '../native/streamProcessor.ts'
import { MAX_EVENTS, MAX_EVENTS_PER_SESSION } from './constants.ts'
import type { NativeAction, NativeState } from './native.types.ts'

function indexSessions(sessions: AgentSession[]): {
  sessions: Record<string, AgentSession>
  sessionOrder: string[]
} {
  const map: Record<string, AgentSession> = {}
  const sessionOrder: string[] = []

  for (const session of sessions) {
    map[session.id] = session
    sessionOrder.push(session.id)
  }

  return { sessions: map, sessionOrder }
}

function trimEvents(events: SessionEvent[]): SessionEvent[] {
  if (events.length <= MAX_EVENTS) {
    return events
  }

  return events.slice(events.length - MAX_EVENTS)
}

function appendEvent(
  events: SessionEvent[],
  eventsBySession: Record<string, SessionEvent[]>,
  event: SessionEvent,
): { events: SessionEvent[]; eventsBySession: Record<string, SessionEvent[]> } {
  const nextEvents = trimEvents([...events, event])
  const sessionEvents = [...(eventsBySession[event.sessionId] ?? []), event]
  const trimmedSessionEvents =
    sessionEvents.length > MAX_EVENTS_PER_SESSION
      ? sessionEvents.slice(sessionEvents.length - MAX_EVENTS_PER_SESSION)
      : sessionEvents

  return {
    events: nextEvents,
    eventsBySession: {
      ...eventsBySession,
      [event.sessionId]: trimmedSessionEvents,
    },
  }
}

function applySnapshot(
  state: NativeState,
  snapshot: AppSnapshot,
  lastSequence: number,
  bootstrapEvents: SessionEvent[] = [],
): NativeState {
  const indexed = indexSessions(snapshot.sessions)
  const events = trimEvents(bootstrapEvents)
  const eventsBySession = Object.fromEntries(
    snapshot.sessions.map((session) => [
      session.id,
      events.filter((event) => event.sessionId === session.id).slice(-MAX_EVENTS_PER_SESSION),
    ]),
  )

  return {
    ...state,
    connection: 'connected',
    protocolVersion: snapshot.protocolVersion,
    lastSequence,
    resyncReason: null,
    errorMessage: null,
    snapshot,
    sessions: indexed.sessions,
    sessionOrder: indexed.sessionOrder,
    events,
    eventsBySession,
    adapters: snapshot.adapters,
    settings: snapshot.settings,
    reducedMotion: snapshot.settings.reducedMotion,
    metrics:
      snapshot.host && snapshot.aggregate
        ? {
            host: snapshot.host,
            aggregate: snapshot.aggregate,
            agents: Object.fromEntries(
              snapshot.sessions.flatMap((session) =>
                session.latestMetric ? [[session.id, session.latestMetric]] : [],
              ),
            ),
          }
        : state.metrics,
    selectedSessionId:
      state.selectedSessionId && indexed.sessions[state.selectedSessionId]
        ? state.selectedSessionId
        : (snapshot.sessions[0]?.id ?? null),
  }
}

function syncSnapshotSessions(
  state: NativeState,
  sessions: Record<string, AgentSession>,
  sessionOrder: string[],
): NativeState['snapshot'] {
  if (!state.snapshot) {
    return state.snapshot
  }

  return {
    ...state.snapshot,
    sessions: sessionOrder
      .map((sessionId) => sessions[sessionId])
      .filter((session): session is AgentSession => session !== undefined),
  }
}

function upsertSession(state: NativeState, session: AgentSession): NativeState {
  const terminal = ['completed', 'failed', 'stale'].includes(session.status)
  const normalizedSession = terminal ? clearLatestMetric(session) : session
  const exists = Boolean(state.sessions[session.id])
  const sessions = { ...state.sessions, [session.id]: normalizedSession }
  const sessionOrder = exists ? state.sessionOrder : [...state.sessionOrder, session.id]

  return {
    ...state,
    sessions,
    sessionOrder,
    snapshot: syncSnapshotSessions(state, sessions, sessionOrder),
    metrics: normalizedSession.latestMetric
      ? {
          host: state.metrics?.host ?? {
            atMs: normalizedSession.latestMetric.atMs,
            cpuHostPercent: 0,
            usedMemoryBytes: 0,
            totalMemoryBytes: 0,
            visibleProcessCount: 0,
            diskReadBytesPerSec: 0,
            diskWriteBytesPerSec: 0,
          },
          aggregate: state.metrics?.aggregate ?? {
            atMs: normalizedSession.latestMetric.atMs,
            cpuCorePercent: 0,
            cpuHostPercent: 0,
            rssBytes: 0,
            runtimeMs: 0,
            processCount: 0,
            readBytesPerSec: 0,
            writeBytesPerSec: 0,
            quality: normalizedSession.latestMetric.quality,
            activeSessions: 0,
            attentionSessions: 0,
          },
          agents: {
            ...(state.metrics?.agents ?? {}),
            [session.id]: normalizedSession.latestMetric,
          },
        }
      : state.metrics,
  }
}

function clearLatestMetric(session: AgentSession): AgentSession {
  const { latestMetric: _latestMetric, ...withoutLatestMetric } = session
  return withoutLatestMetric
}

function mergeMetricsIntoSessions(state: NativeState, frame: NativeState['metrics']): NativeState {
  if (!frame) return state
  const sessions = Object.fromEntries(
    Object.entries(state.sessions).map(([sessionId, session]) => {
      const terminal = ['completed', 'failed', 'stale'].includes(session.status)
      const sample = frame.agents[sessionId]
      return [
        sessionId,
        !terminal && sample ? { ...session, latestMetric: sample } : clearLatestMetric(session),
      ]
    }),
  )
  return {
    ...state,
    sessions,
    snapshot: state.snapshot
      ? {
          ...state.snapshot,
          sessions: state.snapshot.sessions.map(
            (session) => sessions[session.id] ?? clearLatestMetric(session),
          ),
        }
      : state.snapshot,
  }
}

function removeSession(state: NativeState, sessionId: string): NativeState {
  if (!state.sessions[sessionId]) {
    return state
  }

  const { [sessionId]: _removed, ...sessions } = state.sessions
  const eventsBySession = { ...state.eventsBySession }
  delete eventsBySession[sessionId]

  const agents = { ...(state.metrics?.agents ?? {}) }
  delete agents[sessionId]

  const sessionOrder = state.sessionOrder.filter((id) => id !== sessionId)

  return {
    ...state,
    sessions,
    sessionOrder,
    snapshot: syncSnapshotSessions(state, sessions, sessionOrder),
    events: state.events.filter((event) => event.sessionId !== sessionId),
    eventsBySession,
    metrics: state.metrics
      ? {
          ...state.metrics,
          agents,
        }
      : null,
    selectedSessionId: state.selectedSessionId === sessionId ? null : state.selectedSessionId,
  }
}

export function createInitialNativeState(
  clientMode: NativeState['clientMode'] = 'preview',
): NativeState {
  return {
    clientMode,
    connection: 'loading',
    protocolVersion: null,
    lastSequence: null,
    resyncReason: null,
    errorMessage: null,
    snapshot: null,
    sessions: {},
    sessionOrder: [],
    events: [],
    eventsBySession: {},
    metrics: null,
    adapters: [],
    settings: null,
    selectedSessionId: null,
    reducedMotion: false,
    historyRange: '15m',
    historyStatus: 'idle',
    history: null,
    historyError: null,
    displays: [],
    displayStatus: 'idle',
    displayError: null,
  }
}

export function nativeReducer(state: NativeState, action: NativeAction): NativeState {
  switch (action.type) {
    case 'SET_CLIENT_MODE':
      return { ...state, clientMode: action.mode }

    case 'SET_CONNECTION':
      return {
        ...state,
        connection: action.status,
        errorMessage: action.errorMessage ?? null,
      }

    case 'SET_PROTOCOL_INCOMPATIBLE':
      return {
        ...state,
        connection: 'incompatible-protocol',
        protocolVersion: action.received,
        errorMessage: `Protocol ${action.received} is incompatible with renderer protocol ${action.expected}`,
      }

    case 'APPLY_BOOTSTRAP':
      if (action.snapshot.protocolVersion !== PROTOCOL_VERSION) {
        return nativeReducer(state, {
          type: 'SET_PROTOCOL_INCOMPATIBLE',
          received: action.snapshot.protocolVersion,
          expected: PROTOCOL_VERSION,
        })
      }

      return applySnapshot(state, action.snapshot, action.lastSequence, action.events)

    case 'APPLY_FRAME': {
      const { frame } = action

      if (frame.payload.type === 'snapshot') {
        return applySnapshot(state, frame.payload.snapshot, frame.sequence)
      }

      if (frame.payload.type === 'sessionUpsert') {
        return {
          ...upsertSession(state, frame.payload.session),
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'sessionRemove') {
        return {
          ...removeSession(state, frame.payload.sessionId),
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'sessionEvent') {
        const appended = appendEvent(state.events, state.eventsBySession, frame.payload.event)
        return {
          ...state,
          ...appended,
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'metrics') {
        const metrics = mergeMetricsFrame(state.metrics, frame.payload.metrics)
        return {
          ...mergeMetricsIntoSessions(state, metrics),
          metrics,
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'settingsChanged') {
        const settings: PublicSettings = frame.payload.settings
        return {
          ...state,
          settings,
          reducedMotion: settings.reducedMotion,
          snapshot: state.snapshot ? { ...state.snapshot, settings } : state.snapshot,
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'integrationChanged') {
        const { integration } = frame.payload
        const adapters = state.adapters.some((adapter) => adapter.source === integration.source)
          ? state.adapters.map((adapter) =>
              adapter.source === integration.source ? integration : adapter,
            )
          : [...state.adapters, integration]

        return {
          ...state,
          adapters,
          snapshot: state.snapshot ? { ...state.snapshot, adapters } : state.snapshot,
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'resyncRequired') {
        return {
          ...state,
          connection: 'resyncing',
          resyncReason: frame.payload.reason,
          lastSequence: frame.sequence,
        }
      }

      if (frame.payload.type === 'heartbeat') {
        return {
          ...state,
          lastSequence: frame.sequence,
        }
      }

      return state
    }

    case 'SET_RESYNC_REASON':
      return {
        ...state,
        connection: 'resyncing',
        resyncReason: action.reason,
      }

    case 'CLEAR_RESYNC':
      return {
        ...state,
        resyncReason: null,
      }

    case 'SET_SELECTED_SESSION':
      return {
        ...state,
        selectedSessionId: action.sessionId,
      }

    case 'SET_HISTORY_LOADING':
      return {
        ...state,
        historyRange: action.range,
        historyStatus: 'loading',
        historyError: null,
      }

    case 'SET_HISTORY_RANGE':
      return {
        ...state,
        historyRange: action.range,
        historyStatus: action.range === '15m' ? 'ready' : state.historyStatus,
        historyError: null,
      }

    case 'SET_HISTORY':
      return {
        ...state,
        historyRange: action.history.range,
        historyStatus:
          action.history.host.points.length > 0 ||
          action.history.aggregate.points.length > 0 ||
          action.history.agents.some((series) => series.points.length > 0)
            ? 'ready'
            : 'empty',
        history: action.history,
        historyError: null,
      }

    case 'SET_HISTORY_ERROR':
      return {
        ...state,
        historyRange: action.range,
        historyStatus: 'error',
        historyError: action.message,
      }

    case 'SET_DISPLAYS_LOADING':
      return {
        ...state,
        displayStatus: 'loading',
        displayError: null,
      }

    case 'SET_DISPLAYS':
      return {
        ...state,
        displays: action.displays,
        displayStatus: 'ready',
        displayError: null,
      }

    case 'SET_DISPLAYS_ERROR':
      return {
        ...state,
        displays: [],
        displayStatus: 'error',
        displayError: action.message,
      }

    case 'RESET':
      return createInitialNativeState(state.clientMode)

    default:
      return state
  }
}
