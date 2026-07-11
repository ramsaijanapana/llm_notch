use std::collections::VecDeque;

use notch_protocol::MetricSample;

use crate::constants::MAX_HISTORY_SAMPLES_PER_SESSION;

/// Bounded per-session metric history.
#[derive(Debug, Default)]
pub struct SessionHistory {
    samples: VecDeque<MetricSample>,
}

impl SessionHistory {
    pub fn push(&mut self, sample: MetricSample) {
        if self.samples.len() >= MAX_HISTORY_SAMPLES_PER_SESSION {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub fn latest(&self) -> Option<&MetricSample> {
        self.samples.back()
    }

    pub fn samples(&self) -> impl Iterator<Item = &MetricSample> {
        self.samples.iter()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{TreeMetrics, tree_to_sample};
    use notch_protocol::{AttributionQuality, IoQuality, MetricAvailability, MetricQuality};

    fn sample(at_ms: i64) -> MetricSample {
        tree_to_sample(
            at_ms,
            &TreeMetrics {
                cpu_core_percent: 1.0,
                rss_bytes: 1,
                runtime_ms: 1,
                process_count: 1,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality: MetricQuality {
                    attribution: AttributionQuality::Exact,
                    cpu: MetricAvailability::Available,
                    io: IoQuality::Disk,
                    reason: None,
                },
            },
            4,
        )
    }

    #[test]
    fn history_is_bounded() {
        let mut history = SessionHistory::default();
        for i in 0..=MAX_HISTORY_SAMPLES_PER_SESSION {
            history.push(sample(i as i64));
        }
        assert_eq!(history.len(), MAX_HISTORY_SAMPLES_PER_SESSION);
        assert_eq!(
            history.latest().unwrap().at_ms,
            MAX_HISTORY_SAMPLES_PER_SESSION as i64
        );
    }
}
