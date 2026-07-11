use notch_protocol::{AdapterCapabilities, AgentSource, AppSnapshot, SessionEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub snapshot: AppSnapshot,
    pub last_sequence: u64,
    pub events: Vec<SessionEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEventPageResponse {
    pub session_id: String,
    pub events: Vec<SessionEvent>,
    pub next_before_sequence: Option<u64>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPoint {
    pub at_ms: i64,
    pub cpu_host_percent: f64,
    pub cpu_core_percent: f64,
    pub rss_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHistoryResponse {
    pub range: HistoryRange,
    pub since_ms: i64,
    pub end_ms: i64,
    pub host: HistorySeries,
    pub aggregate: HistorySeries,
    pub agents: Vec<AgentHistorySeries>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryRange {
    #[serde(rename = "15m")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "24h")]
    TwentyFourHours,
}

impl HistoryRange {
    pub fn duration_ms(self) -> i64 {
        match self {
            Self::FifteenMinutes => 15 * 60 * 1_000,
            Self::OneHour => 60 * 60 * 1_000,
            Self::TwentyFourHours => 24 * 60 * 60 * 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHistorySeries {
    pub session_id: String,
    #[serde(flatten)]
    pub series: HistorySeries,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySeries {
    pub points: Vec<HistoryPoint>,
    pub actual_first_ms: Option<i64>,
    pub actual_last_ms: Option<i64>,
    pub total_points: u64,
    pub returned_points: usize,
    pub downsampled: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayOption {
    pub id: String,
    pub label: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestedOverlayMode {
    Collapsed,
    Peek,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationHealthStatus {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationHealthEntry {
    pub source: AgentSource,
    pub status: IntegrationHealthStatus,
    pub capabilities: AdapterCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationHealthReport {
    pub checked_at_ms: i64,
    pub overall: IntegrationHealthStatus,
    pub adapters: Vec<IntegrationHealthEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorPreview {
    pub plan_id: String,
    pub source: AgentSource,
    pub summary: String,
    pub expires_at_ms: i64,
}
