//! Resource alert presentation: tray/beacon updates and themed sound (never focus-steals).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use notch_core::{ActiveAlert, AlertKind};
use notch_protocol::{AgentSession, AttentionKind, PublicSettings, SessionStatus, SoundEvent};
use tracing::warn;

use crate::services::sound_theme::{
    SoundNotificationContext, local_minute_now, play_notification_sound, sound_event_for_alert,
    sound_event_for_attention, sound_event_for_session_status,
};

/// Stable key for deduplicating sustained resource alerts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AlertKey {
    kind: AlertKind,
    session_id: Option<String>,
}

/// Tracks which alerts and lifecycle transitions have already triggered optional sound.
#[derive(Debug, Default)]
pub struct AlertNotifier {
    sounded_resource: Mutex<HashSet<AlertKey>>,
    sounded_attention: Mutex<HashSet<String>>,
    lifecycle_status: Mutex<HashMap<String, SessionStatus>>,
}

impl AlertNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns tray indicator text for active resource alerts and plays themed sound once per key.
    pub fn observe(
        &self,
        alerts: &[ActiveAlert],
        sessions: &[AgentSession],
        ctx: &SoundNotificationContext<'_>,
    ) -> Option<String> {
        if ctx.sound_enabled {
            self.play_attention_sounds(alerts, sessions, ctx);
            self.play_resource_sounds(alerts, ctx);
            self.play_lifecycle_sounds(sessions, ctx);
        } else {
            self.reset_sound_state();
        }

        resource_tray_message(alerts)
    }

    fn play_attention_sounds(
        &self,
        alerts: &[ActiveAlert],
        sessions: &[AgentSession],
        ctx: &SoundNotificationContext<'_>,
    ) {
        let mut sounded = self
            .sounded_attention
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for alert in alerts
            .iter()
            .filter(|alert| alert.kind == AlertKind::NewAttention)
        {
            let Some(session_id) = alert.session_id.as_deref() else {
                continue;
            };
            if !sounded.insert(session_id.to_string()) {
                continue;
            }
            let attention = alert.attention.unwrap_or(AttentionKind::Permission);
            let event = sound_event_for_attention(attention);
            let agent = agent_source_for_session(sessions, session_id);
            play_themed_event(ctx, event, agent.as_deref());
        }
    }

    fn play_resource_sounds(&self, alerts: &[ActiveAlert], ctx: &SoundNotificationContext<'_>) {
        let resource: Vec<_> = alerts
            .iter()
            .filter(|alert| {
                matches!(
                    alert.kind,
                    AlertKind::CpuWarn | AlertKind::CpuCritical | AlertKind::MemoryHigh
                )
            })
            .collect();

        if resource.is_empty() {
            self.sounded_resource
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            return;
        }

        let mut sounded = self
            .sounded_resource
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for alert in resource {
            let key = AlertKey {
                kind: alert.kind,
                session_id: alert.session_id.clone(),
            };
            if sounded.insert(key) {
                let event = sound_event_for_alert(alert.kind);
                play_themed_event(ctx, event, None);
            }
        }
    }

    fn play_lifecycle_sounds(&self, sessions: &[AgentSession], ctx: &SoundNotificationContext<'_>) {
        let mut last_status = self
            .lifecycle_status
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let live_ids: HashSet<_> = sessions.iter().map(|session| session.id.as_str()).collect();
        last_status.retain(|session_id, _| live_ids.contains(session_id.as_str()));

        for session in sessions {
            let previous = last_status.insert(session.id.clone(), session.status);
            let Some(previous) = previous else {
                continue;
            };
            if is_terminal_status(previous) || !is_terminal_status(session.status) {
                continue;
            }
            let Some(event) = sound_event_for_session_status(session.status) else {
                continue;
            };
            let agent = Some(agent_source_label(session.source));
            play_themed_event(ctx, event, agent.as_deref());
        }
    }

    fn reset_sound_state(&self) {
        self.sounded_resource
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.sounded_attention
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

fn resource_tray_message(alerts: &[ActiveAlert]) -> Option<String> {
    let resource: Vec<_> = alerts
        .iter()
        .filter(|alert| {
            matches!(
                alert.kind,
                AlertKind::CpuWarn | AlertKind::CpuCritical | AlertKind::MemoryHigh
            )
        })
        .collect();
    if resource.is_empty() {
        return None;
    }
    resource
        .iter()
        .max_by_key(|alert| alert_severity(alert.kind))
        .map(|alert| alert.message.as_str())
        .map(str::to_string)
}

fn play_themed_event(ctx: &SoundNotificationContext<'_>, event: SoundEvent, agent: Option<&str>) {
    match play_notification_sound(ctx, event, agent) {
        Ok(outcome) if outcome.played => {}
        Ok(outcome) => {
            tracing::debug!(?event, reason = ?outcome.reason, "themed notification sound skipped");
        }
        Err(error) => {
            warn!(%error, ?event, "themed notification sound failed");
        }
    }
}

fn agent_source_for_session(sessions: &[AgentSession], session_id: &str) -> Option<String> {
    sessions
        .iter()
        .find(|session| session.id == session_id)
        .map(|session| agent_source_label(session.source))
}

fn agent_source_label(source: notch_protocol::AgentSource) -> String {
    serde_json::to_value(source)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

fn is_terminal_status(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
    )
}

fn alert_severity(kind: AlertKind) -> u8 {
    match kind {
        AlertKind::CpuCritical => 3,
        AlertKind::MemoryHigh => 2,
        AlertKind::CpuWarn => 1,
        AlertKind::NewAttention => 0,
    }
}

pub fn sound_context_from_settings<'a>(
    settings: &'a PublicSettings,
    themes_root: &'a Path,
) -> SoundNotificationContext<'a> {
    SoundNotificationContext {
        sound_enabled: settings.alert_sound_enabled,
        settings,
        themes_root,
        local_minute: local_minute_now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_core::AlertKind;
    use notch_protocol::{AgentSource, SoundRouting};
    use tempfile::TempDir;

    fn resource_alert(kind: AlertKind, message: &str) -> ActiveAlert {
        ActiveAlert {
            kind,
            session_id: None,
            attention: None,
            message: message.into(),
            raised_at_ms: 1,
        }
    }

    fn ctx<'a>(
        settings: &'a PublicSettings,
        themes_root: &'a Path,
    ) -> SoundNotificationContext<'a> {
        SoundNotificationContext {
            sound_enabled: settings.alert_sound_enabled,
            settings,
            themes_root,
            local_minute: 720,
        }
    }

    fn base_settings() -> PublicSettings {
        PublicSettings {
            overlay_enabled: true,
            autostart_enabled: false,
            reduced_motion: false,
            sampling_interval_ms: 1_000,
            selected_display: None,
            show_over_fullscreen: false,
            history_retention_hours: 24,
            alert_sound_enabled: true,
            selected_sound_theme_id: None,
            sound_routing: SoundRouting::default(),
        }
    }

    #[test]
    fn observe_returns_none_when_no_resource_alerts() {
        let notifier = AlertNotifier::new();
        let dir = TempDir::new().expect("tempdir");
        let settings = base_settings();
        let attention = vec![ActiveAlert {
            kind: AlertKind::NewAttention,
            session_id: Some("s1".into()),
            attention: Some(AttentionKind::Permission),
            message: "attention".into(),
            raised_at_ms: 1,
        }];
        let notification = ctx(&settings, dir.path());
        assert!(notifier.observe(&attention, &[], &notification).is_none());
        assert_eq!(notifier.sounded_attention.lock().unwrap().len(), 1);
    }

    #[test]
    fn observe_reports_primary_resource_message() {
        let notifier = AlertNotifier::new();
        let dir = TempDir::new().expect("tempdir");
        let mut settings = base_settings();
        settings.alert_sound_enabled = false;
        let alerts = vec![
            resource_alert(AlertKind::CpuWarn, "Host CPU sustained above 70%"),
            resource_alert(AlertKind::CpuCritical, "Host CPU sustained above 90%"),
        ];
        let notification = ctx(&settings, dir.path());
        assert_eq!(
            notifier.observe(&alerts, &[], &notification).as_deref(),
            Some("Host CPU sustained above 90%")
        );
    }

    #[test]
    fn sound_deduplicates_sustained_alerts() {
        let notifier = AlertNotifier::new();
        let dir = TempDir::new().expect("tempdir");
        let settings = base_settings();
        let alerts = vec![resource_alert(AlertKind::MemoryHigh, "RSS high")];
        let notification = ctx(&settings, dir.path());
        notifier.observe(&alerts, &[], &notification);
        assert_eq!(notifier.sounded_resource.lock().unwrap().len(), 1);
        notifier.observe(&alerts, &[], &notification);
        assert_eq!(notifier.sounded_resource.lock().unwrap().len(), 1);
    }

    #[test]
    fn clearing_resource_alerts_resets_sound_state() {
        let notifier = AlertNotifier::new();
        let dir = TempDir::new().expect("tempdir");
        let settings = base_settings();
        let alerts = vec![resource_alert(AlertKind::CpuWarn, "warn")];
        let notification = ctx(&settings, dir.path());
        notifier.observe(&alerts, &[], &notification);
        notifier.observe(&[], &[], &notification);
        assert!(notifier.sounded_resource.lock().unwrap().is_empty());
    }

    #[test]
    fn lifecycle_sound_fires_only_on_terminal_transition() {
        let notifier = AlertNotifier::new();
        let dir = TempDir::new().expect("tempdir");
        let settings = base_settings();
        let notification = ctx(&settings, dir.path());
        let running = AgentSession {
            id: "s1".into(),
            source: AgentSource::Cursor,
            external_session_id: "ext".into(),
            label: "label".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 1,
            last_event_at_ms: 2,
            ended_at_ms: None,
            process_root: None,
            verified_terminal: None,
            latest_metric: None,
        };
        notifier.observe(&[], &[running.clone()], &notification);
        let completed = AgentSession {
            status: SessionStatus::Completed,
            ended_at_ms: Some(3),
            ..running
        };
        notifier.observe(&[], &[completed], &notification);
        assert_eq!(
            notifier
                .lifecycle_status
                .lock()
                .unwrap()
                .get("s1")
                .copied()
                .unwrap(),
            SessionStatus::Completed
        );
    }

    #[test]
    fn attention_maps_to_existing_sound_events() {
        assert_eq!(
            sound_event_for_attention(AttentionKind::Approval),
            SoundEvent::Approval
        );
        assert_eq!(
            sound_event_for_attention(AttentionKind::Question),
            SoundEvent::Question
        );
        assert_eq!(
            sound_event_for_attention(AttentionKind::Permission),
            SoundEvent::Approval
        );
        assert_eq!(
            sound_event_for_attention(AttentionKind::Error),
            SoundEvent::Failed
        );
    }
}
