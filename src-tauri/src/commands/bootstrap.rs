use std::str::FromStr;
use std::sync::Arc;

use tauri::{State, WebviewWindow, ipc::Channel};
use uuid::Uuid;

use crate::commands::error::CommandError;
use crate::commands::types::{
    AgentHistorySeries, BootstrapResponse, GetHistoryResponse, HistoryPoint, HistoryRange,
    HistorySeries, SessionEventPageResponse,
};
use crate::commands::validation::{validate_session_id, validate_snapshot_session_count};
use crate::state::HostState;

#[tauri::command]
pub fn bootstrap(
    window: WebviewWindow,
    host: State<'_, Arc<HostState>>,
) -> Result<BootstrapResponse, CommandError> {
    validate_native_window(window.label())?;
    let captured = host.snapshot_with_sequence();
    validate_snapshot_session_count(captured.snapshot.sessions.len())?;
    Ok(BootstrapResponse {
        snapshot: captured.snapshot,
        last_sequence: captured.sequence,
        events: captured.events,
    })
}

#[tauri::command]
pub fn subscribe_stream(
    window: WebviewWindow,
    after_sequence: u64,
    on_event: Channel<notch_protocol::StreamFrame>,
    host: State<'_, Arc<HostState>>,
) -> Result<String, CommandError> {
    validate_native_window(window.label())?;
    let sender = Arc::new(move |frame| on_event.send(frame).is_ok());
    host.stream_hub()
        .subscribe(window.label(), after_sequence, sender)
        .map(|subscriber_id| subscriber_id.to_string())
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn unsubscribe_stream(
    window: WebviewWindow,
    subscription_id: String,
    host: State<'_, Arc<HostState>>,
) -> Result<(), CommandError> {
    validate_native_window(window.label())?;
    let subscriber_id = Uuid::from_str(&subscription_id)
        .map_err(|_| CommandError::InvalidRequest("invalid subscription id".into()))?;
    if host.stream_hub().unsubscribe(subscriber_id, window.label()) {
        Ok(())
    } else {
        Err(CommandError::NotFound("stream subscription".into()))
    }
}

#[tauri::command]
pub fn get_history(
    range: HistoryRange,
    host: State<'_, Arc<HostState>>,
) -> Result<GetHistoryResponse, CommandError> {
    const MAX_POINTS_PER_SERIES: usize = 720;
    let end_ms = chrono::Utc::now().timestamp_millis();
    let since_ms = end_ms.saturating_sub(range.duration_ms());
    let history = host
        .persisted_metric_history(since_ms, end_ms, MAX_POINTS_PER_SERIES)
        .map_err(CommandError::Internal)?;
    let map_points = |samples: Vec<notch_protocol::MetricSample>| {
        samples
            .into_iter()
            .map(|sample| HistoryPoint {
                at_ms: sample.at_ms,
                cpu_host_percent: sample.cpu_host_percent,
                cpu_core_percent: sample.cpu_core_percent,
                rss_bytes: sample.rss_bytes,
            })
            .collect()
    };
    let map_series = |series: notch_core::PersistedMetricSeries| {
        let returned_points = series.points.len();
        HistorySeries {
            points: map_points(series.points),
            actual_first_ms: series.actual_first_ms,
            actual_last_ms: series.actual_last_ms,
            total_points: series.total_points,
            returned_points,
            downsampled: series.downsampled,
            truncated: false,
        }
    };
    Ok(GetHistoryResponse {
        range,
        since_ms,
        end_ms,
        host: map_series(history.host),
        aggregate: map_series(history.aggregate),
        agents: history
            .agents
            .into_iter()
            .map(|(session_id, series)| AgentHistorySeries {
                session_id,
                series: map_series(series),
            })
            .collect(),
    })
}

#[tauri::command]
pub fn get_session_events(
    session_id: String,
    before_sequence: Option<u64>,
    limit: Option<u32>,
    host: State<'_, Arc<HostState>>,
) -> Result<SessionEventPageResponse, CommandError> {
    validate_session_id(&session_id)?;
    let limit = limit.unwrap_or(50) as usize;
    if limit == 0 || limit > notch_core::MAX_EVENT_PAGE_SIZE {
        return Err(CommandError::InvalidRequest(format!(
            "event page limit must be 1..={}",
            notch_core::MAX_EVENT_PAGE_SIZE
        )));
    }
    if before_sequence == Some(0) {
        return Err(CommandError::InvalidRequest(
            "beforeSequence must be greater than zero".into(),
        ));
    }
    let page = host
        .session_event_page(&session_id, before_sequence, limit)
        .map_err(CommandError::Internal)?;
    Ok(SessionEventPageResponse {
        session_id,
        events: page.events,
        next_before_sequence: page.next_before_sequence,
        has_more: page.has_more,
    })
}

fn validate_native_window(label: &str) -> Result<(), CommandError> {
    if matches!(label, "overlay" | "dashboard") {
        Ok(())
    } else {
        Err(CommandError::InvalidRequest(
            "command is limited to native application windows".into(),
        ))
    }
}
