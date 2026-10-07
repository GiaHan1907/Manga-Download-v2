//! Coalesced progress/state events for the Downloads UI.
//!
//! Durable state transitions emit immediately; byte/progress updates are
//! rate-limited per task (design §8.2). Every event carries the task
//! `revision` so a UI that sees an older revision ignores the event instead
//! of rolling back newer state.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::domain::{DownloadTaskDto, DownloadTaskEvent};

/// Minimum interval between two progress events for the same task.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// Delivery target for task events. The production sink emits Tauri events;
/// tests collect into a vector. Kept object-safe so the engine never depends
/// on a running app handle.
pub trait TaskSink: Send + Sync + 'static {
    fn publish(&self, event: DownloadTaskEvent);
}

#[derive(Clone, Default)]
#[cfg(test)]
pub struct CollectingSink(pub std::sync::Arc<Mutex<Vec<DownloadTaskEvent>>>);
#[cfg(test)]
impl TaskSink for CollectingSink {
    fn publish(&self, event: DownloadTaskEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// Decides which events leave the engine. The caller supplies live byte
/// counters; this type only throttles progress frames.
pub struct ProgressEmitter {
    sink: std::sync::Arc<dyn TaskSink>,
    min_interval: Duration,
    last_progress: Mutex<HashMap<String, Instant>>,
}

impl ProgressEmitter {
    pub fn new(sink: std::sync::Arc<dyn TaskSink>) -> Self {
        ProgressEmitter {
            sink,
            min_interval: PROGRESS_INTERVAL,
            last_progress: Mutex::new(HashMap::new()),
        }
    }

    /// Always delivers — use for every durable status transition.
    pub fn emit_transition(
        &self,
        task: &DownloadTaskDto,
        downloaded_bytes: u64,
        speed: Option<u64>,
    ) {
        self.reset_gate(&task.id);
        self.deliver(task, downloaded_bytes, speed);
    }

    /// Delivers at most one frame per `min_interval` per task.
    pub fn emit_progress(&self, task: &DownloadTaskDto, downloaded_bytes: u64, speed: Option<u64>) {
        let now = Instant::now();
        let due = {
            let mut gate = self.last_progress.lock().unwrap();
            match gate.get(&task.id) {
                Some(previous) if now.duration_since(*previous) < self.min_interval => false,
                _ => {
                    gate.insert(task.id.clone(), now);
                    true
                }
            }
        };
        if due {
            self.deliver(task, downloaded_bytes, speed);
        }
    }

    fn reset_gate(&self, task_id: &str) {
        self.last_progress.lock().unwrap().remove(task_id);
    }

    fn deliver(&self, task: &DownloadTaskDto, downloaded_bytes: u64, speed: Option<u64>) {
        self.sink.publish(DownloadTaskEvent {
            task: task.clone(),
            downloaded_bytes,
            speed_bytes_per_second: speed,
            occurred_at: crate::domain::now_iso(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto(id: &str, revision: i64) -> DownloadTaskDto {
        DownloadTaskDto {
            id: id.to_string(),
            chapter_id: "c".into(),
            status: "downloading".into(),
            target_format: "cbz".into(),
            priority: 0,
            completed_pages: 0,
            total_pages: 10,
            attempts: 0,
            next_retry_at: None,
            error_code: None,
            error_message: None,
            revision,
            finished_at: None,
            output_relpath: None,
            chapter_title: None,
            manga_title: None,
            chapter_number: None,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn progress_frames_are_coalesced_transitions_are_not() {
        let sink = CollectingSink::default();
        let emitter = ProgressEmitter::new(std::sync::Arc::new(sink.clone()));
        for revision in 0..25 {
            emitter.emit_progress(&dto("tk", revision), revision as u64 * 100, None);
        }
        let burst = sink.0.lock().unwrap().len();
        assert!(
            burst <= 2,
            "burst of 25 progress frames must coalesce, got {burst}"
        );

        for revision in 0..10 {
            emitter.emit_transition(&dto("tk", 100 + revision), 0, None);
        }
        // Every transition delivers exactly one frame on top of the burst.
        assert_eq!(sink.0.lock().unwrap().len(), burst + 10);
    }
}
