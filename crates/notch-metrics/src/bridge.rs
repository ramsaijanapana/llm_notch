#![cfg(feature = "core")]

use async_trait::async_trait;
use notch_core::{CoreError, CoreResult, MetricsSource};
use notch_protocol::{HostMetricSample, MetricSample};

use crate::MetricsEngine;

#[async_trait]
impl MetricsSource for MetricsEngine {
    async fn poll_host_metrics(&self) -> CoreResult<HostMetricSample> {
        self.latest_frame()
            .map(|frame| frame.host)
            .ok_or(CoreError::RepositoryUnavailable)
    }

    async fn poll_session_metrics(&self, session_id: &str) -> CoreResult<Vec<MetricSample>> {
        let history = self.session_history(session_id);
        if history.is_empty() && self.session_latest(session_id).is_none() {
            return Err(CoreError::SessionNotFound(session_id.to_string()));
        }
        Ok(history)
    }
}
