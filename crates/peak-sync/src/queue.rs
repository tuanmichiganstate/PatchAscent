use std::collections::VecDeque;

use patchascent_peak_domain::ParameterId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandClass {
    AtomicSingle,
    AtomicSequence,
    ContinuousParameter,
    NonCoalescible,
    SysexTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueContext {
    pub session_id: Uuid,
    pub patch_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledCommand {
    pub sequence_id: u64,
    pub context: QueueContext,
    pub class: CommandClass,
    pub parameter_id: Option<ParameterId>,
    pub messages: Vec<Vec<u8>>,
    pub enqueued_at_micros: u64,
}

impl ScheduledCommand {
    pub fn validate(&self) -> Result<(), QueueError> {
        if self.messages.is_empty() || self.messages.iter().any(Vec::is_empty) {
            return Err(QueueError::EmptyCommand);
        }
        if self.class == CommandClass::ContinuousParameter && self.parameter_id.is_none() {
            return Err(QueueError::MissingCoalescingKey);
        }
        if self.class == CommandClass::AtomicSingle && self.messages.len() != 1 {
            return Err(QueueError::SingleMessageCount(self.messages.len()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub queue_depth: usize,
    pub coalesced_count: u64,
    pub sent_command_count: u64,
    pub sent_message_count: u64,
    pub cancelled_count: u64,
    pub rejected_count: u64,
    pub error_count: u64,
    pub oldest_item_age_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueueError {
    #[error("scheduled command must contain at least one non-empty MIDI message")]
    EmptyCommand,
    #[error("continuous parameter command must carry a parameter id")]
    MissingCoalescingKey,
    #[error("atomic single command contained {0} messages")]
    SingleMessageCount(usize),
    #[error("output queue is full (maximum depth {max_depth})")]
    Full { max_depth: usize },
}

#[derive(Debug)]
pub struct CommandQueue {
    max_depth: usize,
    pending: VecDeque<ScheduledCommand>,
    metrics: QueueMetrics,
}

impl CommandQueue {
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            pending: VecDeque::with_capacity(max_depth.min(1024)),
            metrics: QueueMetrics::default(),
        }
    }

    pub fn enqueue(&mut self, command: ScheduledCommand) -> Result<(), QueueError> {
        if let Err(error) = command.validate() {
            self.metrics.rejected_count += 1;
            return Err(error);
        }

        if command.class == CommandClass::ContinuousParameter {
            if let Some(existing) = self.pending.iter_mut().find(|existing| {
                existing.class == CommandClass::ContinuousParameter
                    && existing.context == command.context
                    && existing.parameter_id == command.parameter_id
            }) {
                *existing = command;
                self.metrics.coalesced_count += 1;
                self.refresh_depth();
                return Ok(());
            }
        }

        if self.pending.len() >= self.max_depth {
            self.metrics.rejected_count += 1;
            return Err(QueueError::Full {
                max_depth: self.max_depth,
            });
        }
        self.pending.push_back(command);
        self.refresh_depth();
        Ok(())
    }

    /// Returns one complete command. The caller must send all of its messages
    /// before requesting the next command, which makes selector/data sequences
    /// non-interleavable by construction.
    pub fn pop_atomic(&mut self, now_micros: u64) -> Option<ScheduledCommand> {
        let command = self.pending.pop_front()?;
        self.refresh_age(now_micros);
        Some(command)
    }

    pub fn record_sent(&mut self, message_count: usize) {
        self.metrics.sent_command_count += 1;
        self.metrics.sent_message_count += message_count as u64;
    }

    pub fn cancel_session(&mut self, session_id: Uuid) -> usize {
        self.cancel_where(|command| command.context.session_id == session_id)
    }

    pub fn cancel_patch_before(&mut self, context: QueueContext) -> usize {
        self.cancel_where(|command| {
            command.context.session_id == context.session_id
                && command.context.patch_epoch < context.patch_epoch
        })
    }

    pub fn cancel_all(&mut self) -> usize {
        let cancelled = self.pending.len();
        self.pending.clear();
        self.metrics.cancelled_count += cancelled as u64;
        self.refresh_depth();
        cancelled
    }

    pub fn record_error(&mut self) {
        self.metrics.error_count += 1;
    }

    #[must_use]
    pub fn metrics(&self, now_micros: u64) -> QueueMetrics {
        let mut metrics = self.metrics;
        metrics.oldest_item_age_micros = self.pending.front().map_or(0, |command| {
            now_micros.saturating_sub(command.enqueued_at_micros)
        });
        metrics
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    fn cancel_where(&mut self, predicate: impl Fn(&ScheduledCommand) -> bool) -> usize {
        let initial = self.pending.len();
        self.pending.retain(|command| !predicate(command));
        let cancelled = initial - self.pending.len();
        self.metrics.cancelled_count += cancelled as u64;
        self.refresh_depth();
        cancelled
    }

    fn refresh_depth(&mut self) {
        self.metrics.queue_depth = self.pending.len();
        if self.pending.is_empty() {
            self.metrics.oldest_item_age_micros = 0;
        }
    }

    fn refresh_age(&mut self, now_micros: u64) {
        self.refresh_depth();
        self.metrics.oldest_item_age_micros = self.pending.front().map_or(0, |command| {
            now_micros.saturating_sub(command.enqueued_at_micros)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(epoch: u64) -> QueueContext {
        QueueContext {
            session_id: Uuid::nil(),
            patch_epoch: epoch,
        }
    }

    fn command(
        sequence_id: u64,
        class: CommandClass,
        parameter: Option<&str>,
        messages: Vec<Vec<u8>>,
    ) -> ScheduledCommand {
        ScheduledCommand {
            sequence_id,
            context: context(1),
            class,
            parameter_id: parameter.map(|value| ParameterId::new(value).unwrap()),
            messages,
            enqueued_at_micros: sequence_id,
        }
    }

    #[test]
    fn continuous_updates_keep_only_final_value() {
        let mut queue = CommandQueue::new(10);
        for value in 0..=127 {
            queue
                .enqueue(command(
                    u64::from(value),
                    CommandClass::ContinuousParameter,
                    Some("filter.filter_resonance"),
                    vec![vec![0xB0, 79, value]],
                ))
                .unwrap();
        }
        assert_eq!(queue.metrics(200).queue_depth, 1);
        assert_eq!(queue.metrics(200).coalesced_count, 127);
        assert_eq!(
            queue.pop_atomic(200).unwrap().messages,
            vec![vec![0xB0, 79, 127]]
        );
    }

    #[test]
    fn atomic_sequence_is_never_interleaved() {
        let mut queue = CommandQueue::new(10);
        queue
            .enqueue(command(
                1,
                CommandClass::AtomicSequence,
                Some("oscillators.oscillator_1_wave"),
                vec![vec![0xB0, 99, 0], vec![0xB0, 98, 14], vec![0xB0, 6, 4]],
            ))
            .unwrap();
        queue
            .enqueue(command(
                2,
                CommandClass::AtomicSingle,
                None,
                vec![vec![0xB0, 79, 64]],
            ))
            .unwrap();
        let first = queue.pop_atomic(10).unwrap();
        let second = queue.pop_atomic(10).unwrap();
        assert_eq!(first.messages.len(), 3);
        assert_eq!(second.messages.len(), 1);
        assert!(first.sequence_id < second.sequence_id);
    }

    #[test]
    fn patch_transition_cancels_stale_commands() {
        let mut queue = CommandQueue::new(10);
        let mut stale = command(1, CommandClass::AtomicSingle, None, vec![vec![0xB0, 79, 1]]);
        stale.context = context(1);
        queue.enqueue(stale).unwrap();
        assert_eq!(queue.cancel_patch_before(context(2)), 1);
        assert!(queue.is_empty());
    }
}
