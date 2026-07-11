//! Live Tauri stream fan-out with one bounded replay buffer.
//!
//! `notch_core::AppCore` owns sequence assignment. This hub only validates,
//! buffers, and delivers those frames; it never rewrites sequence numbers.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use notch_core::StreamSink;
use notch_protocol::{MAX_STREAM_FRAME_BYTES, StreamFrame};
use parking_lot::Mutex;
use uuid::Uuid;

pub const FRAME_BUFFER_CAPACITY: usize = 1_024;

type FrameSender = Arc<dyn Fn(StreamFrame) -> bool + Send + Sync + 'static>;

#[derive(Debug, Clone)]
pub struct StreamHubConfig {
    pub buffer_capacity: usize,
}

impl Default for StreamHubConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: FRAME_BUFFER_CAPACITY,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("stream frame exceeds {MAX_STREAM_FRAME_BYTES} bytes")]
    FrameTooLarge,
    #[error("stream sequence mismatch: expected {expected}, received {received}")]
    SequenceMismatch { expected: u64, received: u64 },
}

#[derive(Debug, thiserror::Error)]
pub enum SubscribeError {
    #[error("window is not authorized for native stream subscription")]
    InvalidWindowLabel,
    #[error("stream replay gap after sequence {after_sequence}")]
    ReplayGap { after_sequence: u64 },
    #[error("stream channel closed during replay")]
    DeliveryFailed,
}

struct Subscriber {
    window_label: String,
    send: FrameSender,
}

struct HubInner {
    frames: VecDeque<StreamFrame>,
    subscribers: HashMap<Uuid, Subscriber>,
}

pub struct StreamHub {
    config: StreamHubConfig,
    inner: Mutex<HubInner>,
}

impl Default for StreamHub {
    fn default() -> Self {
        Self::new(StreamHubConfig::default())
    }
}

impl StreamHub {
    pub fn new(config: StreamHubConfig) -> Self {
        assert!(config.buffer_capacity > 0, "stream buffer must be non-zero");
        Self {
            config,
            inner: Mutex::new(HubInner {
                frames: VecDeque::new(),
                subscribers: HashMap::new(),
            }),
        }
    }

    pub fn publish(&self, frame: StreamFrame) -> Result<(), PublishError> {
        if serde_json::to_vec(&frame)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX)
            > MAX_STREAM_FRAME_BYTES
        {
            return Err(PublishError::FrameTooLarge);
        }

        let (subscribers, sequence) = {
            let mut inner = self.inner.lock();
            let expected = inner
                .frames
                .back()
                .map(|last| last.sequence.saturating_add(1))
                .unwrap_or(1);
            if frame.sequence != expected {
                return Err(PublishError::SequenceMismatch {
                    expected,
                    received: frame.sequence,
                });
            }

            let sequence = frame.sequence;
            inner.frames.push_back(frame.clone());
            while inner.frames.len() > self.config.buffer_capacity {
                inner.frames.pop_front();
            }
            let subscribers = inner
                .subscribers
                .iter()
                .map(|(id, subscriber)| (*id, Arc::clone(&subscriber.send)))
                .collect::<Vec<_>>();
            (subscribers, sequence)
        };

        let failed = subscribers
            .into_iter()
            .filter_map(|(id, send)| (!send(frame.clone())).then_some(id))
            .collect::<Vec<_>>();
        if !failed.is_empty() {
            let mut inner = self.inner.lock();
            for id in failed {
                inner.subscribers.remove(&id);
            }
        }

        tracing::trace!(sequence, "native stream frame published");
        Ok(())
    }

    /// Atomically replays and registers a live subscriber.
    pub fn subscribe(
        &self,
        window_label: &str,
        after_sequence: u64,
        send: FrameSender,
    ) -> Result<Uuid, SubscribeError> {
        if !matches!(window_label, "overlay" | "dashboard") {
            return Err(SubscribeError::InvalidWindowLabel);
        }

        let mut inner = self.inner.lock();
        let latest = inner.frames.back().map(|frame| frame.sequence).unwrap_or(0);
        let oldest = inner
            .frames
            .front()
            .map(|frame| frame.sequence)
            .unwrap_or(1);
        if after_sequence > latest || (latest > 0 && after_sequence.saturating_add(1) < oldest) {
            return Err(SubscribeError::ReplayGap { after_sequence });
        }

        for frame in inner
            .frames
            .iter()
            .filter(|frame| frame.sequence > after_sequence)
        {
            if !send(frame.clone()) {
                return Err(SubscribeError::DeliveryFailed);
            }
        }

        let id = Uuid::new_v4();
        inner.subscribers.insert(
            id,
            Subscriber {
                window_label: window_label.to_string(),
                send,
            },
        );
        Ok(id)
    }

    pub fn unsubscribe(&self, subscriber_id: Uuid, window_label: &str) -> bool {
        let mut inner = self.inner.lock();
        let authorized = inner
            .subscribers
            .get(&subscriber_id)
            .map(|subscriber| subscriber.window_label == window_label)
            .unwrap_or(false);
        authorized && inner.subscribers.remove(&subscriber_id).is_some()
    }

    pub fn latest_sequence(&self) -> u64 {
        self.inner
            .lock()
            .frames
            .back()
            .map(|frame| frame.sequence)
            .unwrap_or(0)
    }

    pub fn replay_since(&self, sequence: u64) -> Vec<StreamFrame> {
        self.inner
            .lock()
            .frames
            .iter()
            .filter(|frame| frame.sequence > sequence)
            .cloned()
            .collect()
    }

    #[cfg(test)]
    fn subscriber_count(&self) -> usize {
        self.inner.lock().subscribers.len()
    }
}

impl StreamSink for StreamHub {
    fn emit(&self, frame: StreamFrame) {
        if let Err(error) = self.publish(frame) {
            tracing::error!(%error, "dropping invalid native stream frame");
        }
    }

    fn stream_since(&self, sequence: u64) -> Vec<StreamFrame> {
        self.replay_since(sequence)
    }

    fn latest_sequence(&self) -> u64 {
        self.latest_sequence()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::StreamPayload;

    fn heartbeat(sequence: u64) -> StreamFrame {
        StreamFrame {
            sequence,
            emitted_at_ms: sequence as i64,
            payload: StreamPayload::Heartbeat,
        }
    }

    #[test]
    fn preserves_core_sequence_and_replays() {
        let hub = StreamHub::default();
        hub.publish(heartbeat(1)).expect("first frame");
        hub.publish(heartbeat(2)).expect("second frame");
        assert_eq!(hub.latest_sequence(), 2);
        assert_eq!(hub.replay_since(1)[0].sequence, 2);
    }

    #[test]
    fn rejects_duplicate_sequence() {
        let hub = StreamHub::default();
        hub.publish(heartbeat(1)).expect("first frame");
        assert!(matches!(
            hub.publish(heartbeat(1)),
            Err(PublishError::SequenceMismatch { .. })
        ));
    }

    #[test]
    fn subscriber_receives_replay_then_live() {
        let hub = StreamHub::default();
        hub.publish(heartbeat(1)).expect("first frame");
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_for_sender = Arc::clone(&received);
        let id = hub
            .subscribe(
                "overlay",
                0,
                Arc::new(move |frame| {
                    received_for_sender.lock().push(frame.sequence);
                    true
                }),
            )
            .expect("subscribe");
        hub.publish(heartbeat(2)).expect("live frame");
        assert_eq!(*received.lock(), vec![1, 2]);
        assert!(hub.unsubscribe(id, "overlay"));
        assert_eq!(hub.subscriber_count(), 0);
    }

    #[test]
    fn bounded_buffer_reports_replay_gap() {
        let hub = StreamHub::new(StreamHubConfig { buffer_capacity: 2 });
        for sequence in 1..=3 {
            hub.publish(heartbeat(sequence)).expect("frame");
        }
        assert!(matches!(
            hub.subscribe("dashboard", 0, Arc::new(|_| true)),
            Err(SubscribeError::ReplayGap { .. })
        ));
    }
}
