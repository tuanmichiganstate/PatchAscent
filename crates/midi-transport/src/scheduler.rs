use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use patchascent_midi_messages::RawMidiEvent;
use patchascent_peak_sync::{CommandQueue, QueueError, QueueMetrics, ScheduledCommand};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{MidiOutputSink, TransportError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacingProfile {
    pub between_messages_micros: u64,
}

impl Default for PacingProfile {
    fn default() -> Self {
        Self {
            between_messages_micros: 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionReceipt {
    pub sequence_id: u64,
    pub raw_events: Vec<RawMidiEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerStatus {
    pub metrics: QueueMetrics,
    pub last_error: Option<String>,
    pub worker_running: bool,
}

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error("scheduler worker stopped")]
    WorkerStopped,
    #[error("timed out waiting for sequence {sequence_id}")]
    Timeout { sequence_id: u64 },
    #[error("sequence {sequence_id} failed: {detail}")]
    Send { sequence_id: u64, detail: String },
}

#[derive(Debug)]
struct SchedulerState {
    queue: CommandQueue,
    completed: BTreeMap<u64, Result<Vec<RawMidiEvent>, String>>,
    last_error: Option<String>,
    shutdown: bool,
    worker_running: bool,
    clock_started: Instant,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<SchedulerState>,
    wake: Condvar,
}

#[derive(Debug)]
pub struct OutputScheduler {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl OutputScheduler {
    #[must_use]
    pub fn start(sink: impl MidiOutputSink, max_queue_depth: usize, pacing: PacingProfile) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(SchedulerState {
                queue: CommandQueue::new(max_queue_depth.max(1)),
                completed: BTreeMap::new(),
                last_error: None,
                shutdown: false,
                worker_running: true,
                clock_started: Instant::now(),
            }),
            wake: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("patchascent-midi-output".to_owned())
            .spawn(move || run_worker(&worker_shared, sink, pacing))
            .expect("failed to spawn the serialized MIDI output worker");
        Self {
            shared,
            worker: Some(worker),
        }
    }

    pub fn submit(&self, command: ScheduledCommand) -> Result<u64, SchedulerError> {
        let sequence_id = command.sequence_id;
        let mut state = self.shared.state.lock();
        if !state.worker_running || state.shutdown {
            return Err(SchedulerError::WorkerStopped);
        }
        state.queue.enqueue(command)?;
        drop(state);
        self.shared.wake.notify_all();
        Ok(sequence_id)
    }

    pub fn wait(
        &self,
        sequence_id: u64,
        timeout: Duration,
    ) -> Result<SubmissionReceipt, SchedulerError> {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.state.lock();
        loop {
            if let Some(result) = state.completed.remove(&sequence_id) {
                return result
                    .map(|raw_events| SubmissionReceipt {
                        sequence_id,
                        raw_events,
                    })
                    .map_err(|detail| SchedulerError::Send {
                        sequence_id,
                        detail,
                    });
            }
            if !state.worker_running {
                return Err(SchedulerError::WorkerStopped);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(SchedulerError::Timeout { sequence_id });
            }
            self.shared.wake.wait_for(&mut state, deadline - now);
        }
    }

    pub fn cancel_all(&self) -> usize {
        let mut state = self.shared.state.lock();
        state.queue.cancel_all()
    }

    pub fn status(&self) -> SchedulerStatus {
        let state = self.shared.state.lock();
        let now = monotonic_micros(state.clock_started);
        SchedulerStatus {
            metrics: state.queue.metrics(now),
            last_error: state.last_error.clone(),
            worker_running: state.worker_running,
        }
    }

    pub fn shutdown(mut self) {
        self.stop_worker();
    }

    fn stop_worker(&mut self) {
        {
            let mut state = self.shared.state.lock();
            state.shutdown = true;
            state.queue.cancel_all();
        }
        self.shared.wake.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for OutputScheduler {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

fn run_worker(shared: &Shared, mut sink: impl MidiOutputSink, pacing: PacingProfile) {
    loop {
        let command = {
            let mut state = shared.state.lock();
            loop {
                if state.shutdown {
                    state.worker_running = false;
                    shared.wake.notify_all();
                    return;
                }
                let now = monotonic_micros(state.clock_started);
                if let Some(command) = state.queue.pop_atomic(now) {
                    break command;
                }
                shared.wake.wait(&mut state);
            }
        };

        let mut raw_events = Vec::with_capacity(command.messages.len());
        let mut failure: Option<TransportError> = None;
        for (index, message) in command.messages.iter().enumerate() {
            match sink.send(message) {
                Ok(event) => raw_events.push(event),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
            if index + 1 < command.messages.len() && pacing.between_messages_micros > 0 {
                thread::sleep(Duration::from_micros(pacing.between_messages_micros));
            }
        }

        let mut state = shared.state.lock();
        if let Some(error) = failure {
            let detail = error.to_string();
            state.queue.record_error();
            state.last_error = Some(detail.clone());
            state.completed.insert(command.sequence_id, Err(detail));
        } else {
            state.queue.record_sent(command.messages.len());
            state.completed.insert(command.sequence_id, Ok(raw_events));
        }
        drop(state);
        shared.wake.notify_all();
    }
}

fn monotonic_micros(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;
    use patchascent_midi_messages::{MidiDirection, RawMidiEvent};
    use patchascent_peak_sync::{CommandClass, QueueContext};
    use uuid::Uuid;

    use super::*;

    #[derive(Debug)]
    struct FakeSink {
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl MidiOutputSink for FakeSink {
        fn send(&mut self, bytes: &[u8]) -> Result<RawMidiEvent, TransportError> {
            self.sent.lock().push(bytes.to_vec());
            Ok(RawMidiEvent {
                event_id: self.sent.lock().len() as u64,
                monotonic_timestamp_micros: 0,
                wall_clock_timestamp: chrono::Utc::now(),
                port_id: "fake-output".to_owned(),
                port_name: "Fake Peak".to_owned(),
                direction: MidiDirection::Output,
                bytes: bytes.to_vec(),
                session_id: Uuid::nil(),
            })
        }
    }

    fn command(sequence_id: u64, messages: Vec<Vec<u8>>) -> ScheduledCommand {
        ScheduledCommand {
            sequence_id,
            context: QueueContext {
                session_id: Uuid::nil(),
                patch_epoch: 0,
            },
            class: if messages.len() == 1 {
                CommandClass::AtomicSingle
            } else {
                CommandClass::AtomicSequence
            },
            parameter_id: None,
            messages,
            enqueued_at_micros: 0,
        }
    }

    #[test]
    fn sends_every_atomic_sequence_before_the_next_command() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let scheduler = OutputScheduler::start(
            FakeSink {
                sent: Arc::clone(&sent),
            },
            8,
            PacingProfile {
                between_messages_micros: 0,
            },
        );
        scheduler
            .submit(command(
                1,
                vec![vec![0xB0, 99, 0], vec![0xB0, 98, 14], vec![0xB0, 6, 4]],
            ))
            .unwrap();
        scheduler
            .submit(command(2, vec![vec![0xB0, 79, 64]]))
            .unwrap();
        scheduler.wait(1, Duration::from_secs(1)).unwrap();
        scheduler.wait(2, Duration::from_secs(1)).unwrap();
        assert_eq!(
            *sent.lock(),
            vec![
                vec![0xB0, 99, 0],
                vec![0xB0, 98, 14],
                vec![0xB0, 6, 4],
                vec![0xB0, 79, 64],
            ]
        );
    }
}
