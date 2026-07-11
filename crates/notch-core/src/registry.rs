use std::collections::{HashMap, HashSet};

use notch_protocol::{AgentSession, AgentSource, AttentionKind, SessionEvent, SessionStatus};

use crate::constants::{
    BOOTSTRAP_EVENTS_PER_ACTIVE_SESSION, BOOTSTRAP_MAX_EVENTS, MAX_EVENTS_PER_SESSION, MAX_SESSIONS,
};
use crate::domain::{session_id_for, validate_external_session_id};
use crate::error::{CoreError, CoreResult};

fn external_key(source: AgentSource, external_session_id: &str) -> String {
    format!("{source:?}:{external_session_id}")
}

/// In-memory session registry with bounded capacity and external-key lookup.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<String, AgentSession>,
    external_index: HashMap<String, String>,
    events: HashMap<String, Vec<SessionEvent>>,
    idempotency: HashSet<String>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn sessions(&self) -> impl Iterator<Item = &AgentSession> {
        self.sessions.values()
    }

    pub fn get(&self, session_id: &str) -> Option<&AgentSession> {
        self.sessions.get(session_id)
    }

    pub fn get_mut(&mut self, session_id: &str) -> Option<&mut AgentSession> {
        self.sessions.get_mut(session_id)
    }

    pub fn resolve_external(
        &self,
        source: AgentSource,
        external_session_id: &str,
    ) -> Option<&AgentSession> {
        self.external_index
            .get(&external_key(source, external_session_id))
            .and_then(|id| self.sessions.get(id))
    }

    pub fn events_for(&self, session_id: &str) -> &[SessionEvent] {
        self.events
            .get(session_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn bootstrap_events(&self) -> Vec<SessionEvent> {
        let mut selected = Vec::with_capacity(BOOTSTRAP_MAX_EVENTS);
        let mut selected_ids = HashSet::new();
        let mut active_sessions = self
            .sessions
            .values()
            .filter(|session| restore_priority(session) == 0)
            .collect::<Vec<_>>();
        active_sessions.sort_by_key(|session| std::cmp::Reverse(session.last_event_at_ms));

        for session in active_sessions {
            let events = self.events_for(&session.id);
            for event in events
                .iter()
                .rev()
                .take(BOOTSTRAP_EVENTS_PER_ACTIVE_SESSION)
                .rev()
            {
                if selected.len() >= BOOTSTRAP_MAX_EVENTS {
                    break;
                }
                if selected_ids.insert(event.id) {
                    selected.push(event.clone());
                }
            }
        }

        let mut global = self
            .events
            .values()
            .flat_map(|events| events.iter().cloned())
            .collect::<Vec<_>>();
        global.sort_by_key(|event| std::cmp::Reverse((event.occurred_at_ms, event.sequence)));
        for event in global {
            if selected.len() >= BOOTSTRAP_MAX_EVENTS {
                break;
            }
            if selected_ids.insert(event.id) {
                selected.push(event);
            }
        }
        selected.sort_by_key(|event| (event.occurred_at_ms, event.sequence));
        selected
    }

    pub fn check_idempotency(&self, key: &str) -> bool {
        self.idempotency.contains(key)
    }

    pub fn record_idempotency(&mut self, key: String) {
        self.idempotency.insert(key);
    }

    pub fn upsert_session(&mut self, session: AgentSession) -> CoreResult<()> {
        validate_external_session_id(&session.external_session_id)?;
        let key = external_key(session.source, &session.external_session_id);
        self.external_index.insert(key, session.id.clone());
        self.sessions.insert(session.id.clone(), session);
        Ok(())
    }

    pub fn capacity_eviction_candidate(&self) -> CoreResult<Option<String>> {
        if self.sessions.len() < MAX_SESSIONS {
            return Ok(None);
        }
        if let Some(oldest) = self.oldest_terminal_session_id() {
            Ok(Some(oldest))
        } else {
            Err(CoreError::SessionCapacity(MAX_SESSIONS))
        }
    }

    pub fn insert_new_session(
        &mut self,
        source: AgentSource,
        external_session_id: &str,
        build: impl FnOnce(String) -> AgentSession,
    ) -> CoreResult<(AgentSession, bool)> {
        validate_external_session_id(external_session_id)?;
        let key = external_key(source, external_session_id);
        if let Some(existing_id) = self.external_index.get(&key) {
            return Ok((
                self.sessions
                    .get(existing_id)
                    .cloned()
                    .expect("indexed session"),
                true,
            ));
        }

        if self.sessions.len() >= MAX_SESSIONS {
            return Err(CoreError::SessionCapacity(MAX_SESSIONS));
        }
        let id = session_id_for(source, external_session_id);
        let session = build(id);
        self.upsert_session(session.clone())?;
        Ok((session, false))
    }

    pub fn append_event(&mut self, event: SessionEvent) -> CoreResult<()> {
        let session_id = event.session_id.clone();
        if !self.sessions.contains_key(&session_id) {
            return Err(CoreError::SessionNotFound(session_id));
        }

        let events = self.events.entry(session_id).or_default();
        if let Some(last) = events.last() {
            if event.sequence != last.sequence + 1 {
                return Err(CoreError::IngestRejected(format!(
                    "expected event sequence {}, got {}",
                    last.sequence + 1,
                    event.sequence
                )));
            }
        } else if event.sequence != 1 {
            return Err(CoreError::IngestRejected(format!(
                "first event sequence must be 1, got {}",
                event.sequence
            )));
        }

        events.push(event);
        if events.len() > MAX_EVENTS_PER_SESSION {
            let overflow = events.len() - MAX_EVENTS_PER_SESSION;
            events.drain(0..overflow);
        }
        Ok(())
    }

    pub fn next_event_sequence(&self, session_id: &str) -> u64 {
        self.events
            .get(session_id)
            .and_then(|events| events.last().map(|e| e.sequence + 1))
            .unwrap_or(1)
    }

    pub fn remove_session(&mut self, session_id: &str) -> CoreResult<()> {
        let session = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| CoreError::SessionNotFound(session_id.to_string()))?;
        self.external_index
            .remove(&external_key(session.source, &session.external_session_id));
        self.events.remove(session_id);
        Ok(())
    }

    pub fn restore(
        &mut self,
        mut sessions: Vec<AgentSession>,
        mut events: Vec<SessionEvent>,
    ) -> CoreResult<()> {
        self.sessions.clear();
        self.external_index.clear();
        self.events.clear();

        sessions.sort_by(|left, right| {
            restore_priority(left)
                .cmp(&restore_priority(right))
                .then_with(|| right.last_event_at_ms.cmp(&left.last_event_at_ms))
        });
        sessions.truncate(MAX_SESSIONS);
        for session in sessions {
            self.upsert_session(session)?;
        }

        events.retain(|event| self.sessions.contains_key(&event.session_id));
        events.sort_by_key(|event| (event.session_id.clone(), event.sequence));
        for event in events {
            let sid = event.session_id.clone();
            let session_events = self.events.entry(sid).or_default();
            session_events.push(event);
            if session_events.len() > MAX_EVENTS_PER_SESSION {
                let overflow = session_events.len() - MAX_EVENTS_PER_SESSION;
                session_events.drain(0..overflow);
            }
        }
        Ok(())
    }

    pub fn clear_latest_metrics(&mut self) {
        for session in self.sessions.values_mut() {
            session.latest_metric = None;
        }
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    fn oldest_terminal_session_id(&self) -> Option<String> {
        use crate::domain::is_terminal;
        self.sessions
            .values()
            .filter(|s| is_terminal(s.status))
            .min_by_key(|s| s.ended_at_ms.unwrap_or(s.last_event_at_ms))
            .map(|s| s.id.clone())
    }
}

fn restore_priority(session: &AgentSession) -> u8 {
    if !matches!(
        session.status,
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
    ) || session.attention != AttentionKind::None
    {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{AgentSource, AttentionKind, SessionStatus};

    fn sample_session(id: &str, external: &str) -> AgentSession {
        AgentSession {
            id: id.into(),
            source: AgentSource::Cursor,
            external_session_id: external.into(),
            label: "test".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 1,
            last_event_at_ms: 1,
            ended_at_ms: None,
            process_root: None,
            latest_metric: None,
        }
    }

    #[test]
    fn external_key_is_idempotent() {
        let mut reg = SessionRegistry::new();
        let (a, replay) = reg
            .insert_new_session(AgentSource::Cursor, "ext-1", |id| {
                sample_session(&id, "ext-1")
            })
            .unwrap();
        let (b, replay2) = reg
            .insert_new_session(AgentSource::Cursor, "ext-1", |id| {
                sample_session(&id, "ext-1")
            })
            .unwrap();
        assert!(!replay);
        assert!(replay2);
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn bootstrap_events_remain_globally_bounded_at_max_session_load() {
        let mut registry = SessionRegistry::new();
        for session_index in 0..MAX_SESSIONS {
            let session_id = format!("session-{session_index}");
            registry
                .upsert_session(sample_session(
                    &session_id,
                    &format!("external-{session_index}"),
                ))
                .unwrap();
            for sequence in 1..=100_u64 {
                registry
                    .append_event(SessionEvent {
                        id: uuid::Uuid::new_v4(),
                        session_id: session_id.clone(),
                        sequence,
                        occurred_at_ms: session_index as i64 * 1_000 + sequence as i64,
                        kind: notch_protocol::SessionEventKind::Status,
                        level: notch_protocol::EventLevel::Info,
                        summary: "bounded bootstrap event".into(),
                        tool_name: None,
                    })
                    .unwrap();
            }
        }

        let events = registry.bootstrap_events();
        assert_eq!(events.len(), BOOTSTRAP_MAX_EVENTS);
        assert!(events.windows(2).all(|window| {
            (window[0].occurred_at_ms, window[0].sequence)
                <= (window[1].occurred_at_ms, window[1].sequence)
        }));
        let serialized = serde_json::to_vec(&events).unwrap();
        assert!(
            serialized.len() < 128 * 1024,
            "bounded bootstrap should remain small, got {} bytes",
            serialized.len()
        );
    }
}
