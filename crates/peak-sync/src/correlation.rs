use std::collections::VecDeque;

use patchascent_peak_domain::ParameterId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundCorrelation {
    pub sequence_id: u64,
    pub port_id: String,
    pub channel_one_based: u8,
    pub parameter_id: ParameterId,
    pub raw_value: i32,
    pub expires_at_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundSemanticValue {
    pub port_id: String,
    pub channel_one_based: u8,
    pub parameter_id: ParameterId,
    pub raw_value: i32,
    pub timestamp_micros: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrelationOutcome {
    Confirmation { sequence_id: u64 },
    HardwareOriginated,
}

#[derive(Debug)]
pub struct CorrelationTracker {
    max_entries: usize,
    entries: VecDeque<OutboundCorrelation>,
    expired_count: u64,
}

impl CorrelationTracker {
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        let max_entries = max_entries.max(1);
        Self {
            max_entries,
            entries: VecDeque::with_capacity(max_entries.min(1024)),
            expired_count: 0,
        }
    }

    pub fn insert(&mut self, correlation: OutboundCorrelation) {
        while self.entries.len() >= self.max_entries {
            self.entries.pop_front();
            self.expired_count += 1;
        }
        self.entries.push_back(correlation);
    }

    pub fn correlate(&mut self, inbound: &InboundSemanticValue) -> CorrelationOutcome {
        self.expire(inbound.timestamp_micros);
        let position = self.entries.iter().position(|candidate| {
            candidate.port_id == inbound.port_id
                && candidate.channel_one_based == inbound.channel_one_based
                && candidate.parameter_id == inbound.parameter_id
                && candidate.raw_value == inbound.raw_value
        });
        if let Some(position) = position {
            let correlation = self
                .entries
                .remove(position)
                .expect("position came from the same queue");
            CorrelationOutcome::Confirmation {
                sequence_id: correlation.sequence_id,
            }
        } else {
            CorrelationOutcome::HardwareOriginated
        }
    }

    pub fn expire(&mut self, now_micros: u64) {
        let initial = self.entries.len();
        self.entries
            .retain(|entry| entry.expires_at_micros >= now_micros);
        self.expired_count += (initial - self.entries.len()) as u64;
    }

    #[must_use]
    pub const fn expired_count(&self) -> u64 {
        self.expired_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbound(raw_value: i32, timestamp_micros: u64) -> InboundSemanticValue {
        InboundSemanticValue {
            port_id: "port".to_owned(),
            channel_one_based: 1,
            parameter_id: ParameterId::new("filter.filter_resonance").unwrap(),
            raw_value,
            timestamp_micros,
        }
    }

    #[test]
    fn consumes_only_the_matching_confirmation() {
        let mut tracker = CorrelationTracker::new(10);
        tracker.insert(OutboundCorrelation {
            sequence_id: 7,
            port_id: "port".to_owned(),
            channel_one_based: 1,
            parameter_id: ParameterId::new("filter.filter_resonance").unwrap(),
            raw_value: 64,
            expires_at_micros: 100,
        });
        assert_eq!(
            tracker.correlate(&inbound(64, 50)),
            CorrelationOutcome::Confirmation { sequence_id: 7 }
        );
        assert_eq!(
            tracker.correlate(&inbound(65, 51)),
            CorrelationOutcome::HardwareOriginated
        );
    }

    #[test]
    fn late_echo_is_not_suppressed_forever() {
        let mut tracker = CorrelationTracker::new(10);
        tracker.insert(OutboundCorrelation {
            sequence_id: 7,
            port_id: "port".to_owned(),
            channel_one_based: 1,
            parameter_id: ParameterId::new("filter.filter_resonance").unwrap(),
            raw_value: 64,
            expires_at_micros: 10,
        });
        assert_eq!(
            tracker.correlate(&inbound(64, 11)),
            CorrelationOutcome::HardwareOriginated
        );
        assert_eq!(tracker.expired_count(), 1);
    }

    #[test]
    fn zero_capacity_is_safely_clamped() {
        let mut tracker = CorrelationTracker::new(0);
        tracker.insert(OutboundCorrelation {
            sequence_id: 7,
            port_id: "port".to_owned(),
            channel_one_based: 1,
            parameter_id: ParameterId::new("filter.filter_resonance").unwrap(),
            raw_value: 64,
            expires_at_micros: 100,
        });
        assert_eq!(
            tracker.correlate(&inbound(64, 50)),
            CorrelationOutcome::Confirmation { sequence_id: 7 }
        );
    }
}
