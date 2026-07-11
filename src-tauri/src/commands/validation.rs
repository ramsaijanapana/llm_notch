use notch_protocol::{
    AgentSource, MAX_SESSION_ID_LEN, MAX_SESSION_LABEL_LEN, MAX_SNAPSHOT_SESSIONS, PublicSettings,
};

use crate::commands::error::CommandError;

pub const MIN_SAMPLING_INTERVAL_MS: u64 = 250;
pub const MAX_SAMPLING_INTERVAL_MS: u64 = 60_000;
pub const MIN_HISTORY_RETENTION_HOURS: u32 = 1;
pub const MAX_HISTORY_RETENTION_HOURS: u32 = 24 * 30;
pub const MAX_PLAN_ID_LEN: usize = 128;
pub const MAX_ACCELERATOR_LEN: usize = 64;
#[cfg(test)]
const MAX_WINDOW_LABEL_LEN: usize = 64;

#[cfg(test)]
fn validate_window_label(label: &str) -> Result<(), CommandError> {
    if label.is_empty() || label.len() > MAX_WINDOW_LABEL_LEN {
        return Err(CommandError::InvalidRequest("invalid window label".into()));
    }
    if !label
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(CommandError::InvalidRequest("invalid window label".into()));
    }
    Ok(())
}

pub fn validate_session_id(session_id: &str) -> Result<(), CommandError> {
    if session_id.is_empty() || session_id.len() > MAX_SESSION_ID_LEN {
        return Err(CommandError::InvalidRequest("invalid session id".into()));
    }
    Ok(())
}

pub fn validate_plan_id(plan_id: &str) -> Result<(), CommandError> {
    if plan_id.is_empty() || plan_id.len() > MAX_PLAN_ID_LEN {
        return Err(CommandError::InvalidRequest("invalid plan id".into()));
    }
    if !plan_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(CommandError::InvalidRequest("invalid plan id".into()));
    }
    Ok(())
}

pub fn validate_accelerator(accelerator: &str) -> Result<(), CommandError> {
    if accelerator.is_empty() || accelerator.len() > MAX_ACCELERATOR_LEN {
        return Err(CommandError::InvalidRequest("invalid accelerator".into()));
    }
    Ok(())
}

pub fn validate_settings(settings: &PublicSettings) -> Result<(), CommandError> {
    if settings.sampling_interval_ms < MIN_SAMPLING_INTERVAL_MS
        || settings.sampling_interval_ms > MAX_SAMPLING_INTERVAL_MS
    {
        return Err(CommandError::InvalidRequest(
            "samplingIntervalMs out of range".into(),
        ));
    }
    if settings.history_retention_hours < MIN_HISTORY_RETENTION_HOURS
        || settings.history_retention_hours > MAX_HISTORY_RETENTION_HOURS
    {
        return Err(CommandError::InvalidRequest(
            "historyRetentionHours out of range".into(),
        ));
    }
    if let Some(display) = &settings.selected_display {
        if display.is_empty() || display.len() > MAX_SESSION_LABEL_LEN {
            return Err(CommandError::InvalidRequest(
                "invalid selectedDisplay".into(),
            ));
        }
    }
    Ok(())
}

pub fn validate_platform_settings(
    settings: &PublicSettings,
    windows_target: bool,
) -> Result<(), CommandError> {
    if windows_target && settings.show_over_fullscreen {
        return Err(CommandError::InvalidRequest(
            "showOverFullscreen is unsupported on Windows; normal topmost overlay behavior remains enabled"
                .into(),
        ));
    }
    Ok(())
}

pub fn validate_agent_source(source: AgentSource) -> Result<AgentSource, CommandError> {
    match source {
        AgentSource::Unknown => Err(CommandError::InvalidRequest("invalid agent source".into())),
        other => Ok(other),
    }
}

pub fn validate_snapshot_session_count(count: usize) -> Result<(), CommandError> {
    if count > MAX_SNAPSHOT_SESSIONS {
        return Err(CommandError::InvalidRequest("too many sessions".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_settings() -> PublicSettings {
        PublicSettings {
            overlay_enabled: true,
            autostart_enabled: false,
            reduced_motion: false,
            sampling_interval_ms: 1_000,
            selected_display: None,
            show_over_fullscreen: false,
            history_retention_hours: 24,
        }
    }

    #[test]
    fn rejects_invalid_window_labels() {
        assert!(validate_window_label("").is_err());
        assert!(validate_window_label("has space").is_err());
        assert!(validate_window_label("overlay").is_ok());
    }

    #[test]
    fn rejects_out_of_range_sampling_interval() {
        let mut settings = base_settings();
        settings.sampling_interval_ms = 10;
        assert!(validate_settings(&settings).is_err());
        settings.sampling_interval_ms = 1_000;
        assert!(validate_settings(&settings).is_ok());
    }

    #[test]
    fn windows_rejects_unsupported_fullscreen_preference() {
        let mut settings = base_settings();
        settings.show_over_fullscreen = true;
        assert!(validate_platform_settings(&settings, true).is_err());
        assert!(validate_platform_settings(&settings, false).is_ok());
        settings.show_over_fullscreen = false;
        assert!(validate_platform_settings(&settings, true).is_ok());
    }

    #[test]
    fn rejects_unknown_agent_source_for_integrations() {
        assert!(validate_agent_source(AgentSource::Unknown).is_err());
        assert!(validate_agent_source(AgentSource::Cursor).is_ok());
    }

    #[test]
    fn plan_id_rejects_path_like_values() {
        assert!(validate_plan_id("/etc/passwd").is_err());
        assert!(validate_plan_id("plan-123").is_ok());
    }
}
