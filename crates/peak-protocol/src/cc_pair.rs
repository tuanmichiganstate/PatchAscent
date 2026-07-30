use patchascent_midi_messages::{ChannelMessage, MidiChannel};
use serde::{Deserialize, Serialize};

/// Interface reserved for the codec established by HV-007/HV-008.
///
/// There is deliberately no implementation in the default codebase.
pub trait CcPairCodec {
    type Error;

    fn encode(&self, raw: u16) -> Result<[[u8; 2]; 2], Self::Error>;
    fn ingest(&mut self, controller: u8, value: u8, timestamp_micros: u64) -> Option<u16>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairMember {
    First,
    Second,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CcPairObservation {
    pub channel: MidiChannel,
    pub member: PairMember,
    pub controller: u8,
    pub value: u8,
    pub timestamp_micros: u64,
    pub prior_other_member: Option<(u8, u64)>,
}

/// Receive-only paired-CC analysis. It records relationships without deriving
/// or claiming an encoder.
#[derive(Debug, Clone)]
pub struct CcPairAnalyzer {
    first_controller: u8,
    second_controller: u8,
    last_first: [Option<(u8, u64)>; 16],
    last_second: [Option<(u8, u64)>; 16],
}

impl CcPairAnalyzer {
    #[must_use]
    pub fn new(first_controller: u8, second_controller: u8) -> Self {
        Self {
            first_controller,
            second_controller,
            last_first: [None; 16],
            last_second: [None; 16],
        }
    }

    pub fn ingest(
        &mut self,
        message: &ChannelMessage,
        timestamp_micros: u64,
    ) -> Option<CcPairObservation> {
        let ChannelMessage::ControlChange {
            channel,
            controller,
            value,
        } = message
        else {
            return None;
        };
        let index = usize::from(channel.zero_based());
        if *controller == self.first_controller {
            let observation = CcPairObservation {
                channel: *channel,
                member: PairMember::First,
                controller: *controller,
                value: *value,
                timestamp_micros,
                prior_other_member: self.last_second[index],
            };
            self.last_first[index] = Some((*value, timestamp_micros));
            Some(observation)
        } else if *controller == self.second_controller {
            let observation = CcPairObservation {
                channel: *channel,
                member: PairMember::Second,
                controller: *controller,
                value: *value,
                timestamp_micros,
                prior_other_member: self.last_first[index],
            };
            self.last_second[index] = Some((*value, timestamp_micros));
            Some(observation)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observes_both_members_without_claiming_a_value() {
        let channel = MidiChannel::from_one_based(1).unwrap();
        let mut analyzer = CcPairAnalyzer::new(29, 61);
        let first = ChannelMessage::ControlChange {
            channel,
            controller: 29,
            value: 10,
        };
        let second = ChannelMessage::ControlChange {
            channel,
            controller: 61,
            value: 11,
        };
        assert_eq!(
            analyzer.ingest(&first, 100).unwrap().prior_other_member,
            None
        );
        assert_eq!(
            analyzer.ingest(&second, 110).unwrap().prior_other_member,
            Some((10, 100))
        );
    }
}
