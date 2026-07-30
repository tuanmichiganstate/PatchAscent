#[cfg(feature = "nrpn_candidate_experimental")]
use patchascent_midi_messages::{encode_control_change, MidiMessageError};
use patchascent_midi_messages::{ChannelMessage, MidiChannel};
use serde::{Deserialize, Serialize};

#[cfg(feature = "nrpn_candidate_experimental")]
use crate::S1LiveEditAcknowledgement;

pub const OSCILLATOR_1_WAVE_NRPN: (u8, u8) = (0, 14);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NrpnSelector {
    #[default]
    None,
    Nrpn {
        msb: Option<u8>,
        lsb: Option<u8>,
    },
    Rpn {
        msb: Option<u8>,
        lsb: Option<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NrpnValue {
    pub data_msb: u8,
    pub data_lsb: Option<u8>,
}

impl NrpnValue {
    #[must_use]
    pub const fn value_7bit(self) -> u8 {
        self.data_msb
    }

    #[must_use]
    pub fn value_14bit(self) -> u16 {
        u16::from(self.data_msb) << 7 | u16::from(self.data_lsb.unwrap_or(0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NrpnDiagnosticEvent {
    SelectorChanged {
        channel: MidiChannel,
        selector: NrpnSelector,
    },
    DataEntry {
        channel: MidiChannel,
        selector: NrpnSelector,
        value: NrpnValue,
    },
    DataIncrement {
        channel: MidiChannel,
        selector: NrpnSelector,
        amount: u8,
    },
    DataDecrement {
        channel: MidiChannel,
        selector: NrpnSelector,
        amount: u8,
    },
    StateExpired {
        channel: MidiChannel,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NrpnParserConfig {
    pub state_expiry_micros: u64,
}

impl Default for NrpnParserConfig {
    fn default() -> Self {
        Self {
            state_expiry_micros: 500_000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ChannelState {
    selector: NrpnSelector,
    data_msb: Option<u8>,
    data_lsb: Option<u8>,
    last_timestamp_micros: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct NrpnParser {
    config: NrpnParserConfig,
    channels: [ChannelState; 16],
}

impl NrpnParser {
    #[must_use]
    pub fn new(config: NrpnParserConfig) -> Self {
        Self {
            config,
            channels: [ChannelState::default(); 16],
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the MIDI NRPN/RPN controller state machine is kept in one exhaustive match"
    )]
    pub fn ingest(
        &mut self,
        message: &ChannelMessage,
        timestamp_micros: u64,
    ) -> Vec<NrpnDiagnosticEvent> {
        let ChannelMessage::ControlChange {
            channel,
            controller,
            value,
        } = message
        else {
            return Vec::new();
        };
        let state = &mut self.channels[usize::from(channel.zero_based())];
        let mut events = Vec::new();

        if let Some(previous) = state.last_timestamp_micros {
            let expired = timestamp_micros < previous
                || timestamp_micros.saturating_sub(previous) > self.config.state_expiry_micros;
            if expired && !matches!(state.selector, NrpnSelector::None) {
                *state = ChannelState::default();
                events.push(NrpnDiagnosticEvent::StateExpired { channel: *channel });
            }
        }
        state.last_timestamp_micros = Some(timestamp_micros);

        match *controller {
            99 => {
                let lsb = match state.selector {
                    NrpnSelector::Nrpn { lsb, .. } => lsb,
                    _ => None,
                };
                state.selector = NrpnSelector::Nrpn {
                    msb: Some(*value),
                    lsb,
                };
                state.data_msb = None;
                state.data_lsb = None;
                normalize_null_selector(state);
                events.push(selector_event(*channel, state.selector));
            }
            98 => {
                let msb = match state.selector {
                    NrpnSelector::Nrpn { msb, .. } => msb,
                    _ => None,
                };
                state.selector = NrpnSelector::Nrpn {
                    msb,
                    lsb: Some(*value),
                };
                state.data_msb = None;
                state.data_lsb = None;
                normalize_null_selector(state);
                events.push(selector_event(*channel, state.selector));
            }
            101 => {
                let lsb = match state.selector {
                    NrpnSelector::Rpn { lsb, .. } => lsb,
                    _ => None,
                };
                state.selector = NrpnSelector::Rpn {
                    msb: Some(*value),
                    lsb,
                };
                state.data_msb = None;
                state.data_lsb = None;
                normalize_null_selector(state);
                events.push(selector_event(*channel, state.selector));
            }
            100 => {
                let msb = match state.selector {
                    NrpnSelector::Rpn { msb, .. } => msb,
                    _ => None,
                };
                state.selector = NrpnSelector::Rpn {
                    msb,
                    lsb: Some(*value),
                };
                state.data_msb = None;
                state.data_lsb = None;
                normalize_null_selector(state);
                events.push(selector_event(*channel, state.selector));
            }
            6 => {
                state.data_msb = Some(*value);
                state.data_lsb = None;
                events.push(NrpnDiagnosticEvent::DataEntry {
                    channel: *channel,
                    selector: state.selector,
                    value: NrpnValue {
                        data_msb: *value,
                        data_lsb: None,
                    },
                });
            }
            38 => {
                state.data_lsb = Some(*value);
                if let Some(data_msb) = state.data_msb {
                    events.push(NrpnDiagnosticEvent::DataEntry {
                        channel: *channel,
                        selector: state.selector,
                        value: NrpnValue {
                            data_msb,
                            data_lsb: Some(*value),
                        },
                    });
                }
            }
            96 => events.push(NrpnDiagnosticEvent::DataIncrement {
                channel: *channel,
                selector: state.selector,
                amount: (*value).max(1),
            }),
            97 => events.push(NrpnDiagnosticEvent::DataDecrement {
                channel: *channel,
                selector: state.selector,
                amount: (*value).max(1),
            }),
            _ => {}
        }
        events
    }
}

fn selector_event(channel: MidiChannel, selector: NrpnSelector) -> NrpnDiagnosticEvent {
    NrpnDiagnosticEvent::SelectorChanged { channel, selector }
}

fn normalize_null_selector(state: &mut ChannelState) {
    let is_null = matches!(
        state.selector,
        NrpnSelector::Nrpn {
            msb: Some(127),
            lsb: Some(127)
        } | NrpnSelector::Rpn {
            msb: Some(127),
            lsb: Some(127)
        }
    );
    if is_null {
        state.selector = NrpnSelector::None;
    }
}

#[cfg(feature = "nrpn_candidate_experimental")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NrpnEncodingStrategy {
    pub include_data_lsb: bool,
    pub terminate_with_null_selector: bool,
}

/// Generic standards-based candidate. This is not evidence that Peak accepts
/// every option and therefore exists only in experimental builds.
#[cfg(feature = "nrpn_candidate_experimental")]
#[allow(
    clippy::similar_names,
    reason = "MSB and LSB are the exact MIDI NRPN field names"
)]
pub fn encode_candidate_nrpn(
    channel: MidiChannel,
    parameter_msb: u8,
    parameter_lsb: u8,
    value_14bit: u16,
    strategy: NrpnEncodingStrategy,
) -> Result<Vec<[u8; 3]>, MidiMessageError> {
    if value_14bit > 0x3FFF {
        return Err(MidiMessageError::InvalidSevenBitValue {
            field: "14-bit value MSB",
            value: u8::MAX,
        });
    }
    let data_msb = ((value_14bit >> 7) & 0x7F) as u8;
    let data_lsb = (value_14bit & 0x7F) as u8;
    let mut messages = vec![
        encode_control_change(channel, 99, parameter_msb)?,
        encode_control_change(channel, 98, parameter_lsb)?,
        encode_control_change(channel, 6, data_msb)?,
    ];
    if strategy.include_data_lsb {
        messages.push(encode_control_change(channel, 38, data_lsb)?);
    }
    if strategy.terminate_with_null_selector {
        messages.push(encode_control_change(channel, 99, 127)?);
        messages.push(encode_control_change(channel, 98, 127)?);
    }
    Ok(messages)
}

/// The only initially allowlisted candidate NRPN write: Oscillator 1 Wave 0:14.
#[cfg(feature = "nrpn_candidate_experimental")]
pub fn encode_oscillator_1_wave_candidate(
    channel: MidiChannel,
    value: u8,
    strategy: NrpnEncodingStrategy,
    _acknowledgement: S1LiveEditAcknowledgement,
) -> Result<Vec<[u8; 3]>, MidiMessageError> {
    if value > 4 {
        return Err(MidiMessageError::InvalidSevenBitValue {
            field: "Oscillator 1 Wave",
            value,
        });
    }
    encode_candidate_nrpn(
        channel,
        OSCILLATOR_1_WAVE_NRPN.0,
        OSCILLATOR_1_WAVE_NRPN.1,
        u16::from(value) << 7,
        strategy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cc(controller: u8, value: u8) -> ChannelMessage {
        ChannelMessage::ControlChange {
            channel: MidiChannel::from_one_based(1).unwrap(),
            controller,
            value,
        }
    }

    #[test]
    fn parses_nrpn_selector_and_7bit_value() {
        let mut parser = NrpnParser::new(NrpnParserConfig::default());
        parser.ingest(&cc(99, 0), 1);
        parser.ingest(&cc(98, 14), 2);
        let events = parser.ingest(&cc(6, 4), 3);
        assert_eq!(
            events,
            vec![NrpnDiagnosticEvent::DataEntry {
                channel: MidiChannel::from_one_based(1).unwrap(),
                selector: NrpnSelector::Nrpn {
                    msb: Some(0),
                    lsb: Some(14)
                },
                value: NrpnValue {
                    data_msb: 4,
                    data_lsb: None
                }
            }]
        );
    }

    #[test]
    fn rpn_selection_cancels_nrpn_selection() {
        let mut parser = NrpnParser::new(NrpnParserConfig::default());
        parser.ingest(&cc(99, 1), 1);
        parser.ingest(&cc(98, 2), 2);
        parser.ingest(&cc(101, 3), 3);
        let events = parser.ingest(&cc(100, 4), 4);
        assert!(matches!(
            events.as_slice(),
            [NrpnDiagnosticEvent::SelectorChanged {
                selector: NrpnSelector::Rpn {
                    msb: Some(3),
                    lsb: Some(4)
                },
                ..
            }]
        ));
    }

    #[test]
    fn expires_selector_state() {
        let mut parser = NrpnParser::new(NrpnParserConfig {
            state_expiry_micros: 10,
        });
        parser.ingest(&cc(99, 0), 1);
        let events = parser.ingest(&cc(6, 2), 20);
        assert!(matches!(
            events.as_slice(),
            [
                NrpnDiagnosticEvent::StateExpired { .. },
                NrpnDiagnosticEvent::DataEntry {
                    selector: NrpnSelector::None,
                    ..
                }
            ]
        ));
    }

    #[cfg(feature = "nrpn_candidate_experimental")]
    #[test]
    fn candidate_sequence_is_atomic_data() {
        let channel = MidiChannel::from_one_based(1).unwrap();
        let acknowledgement = S1LiveEditAcknowledgement::from_cli_flag(true).unwrap();
        let messages = encode_oscillator_1_wave_candidate(
            channel,
            4,
            NrpnEncodingStrategy::default(),
            acknowledgement,
        )
        .unwrap();
        assert_eq!(messages, vec![[0xB0, 99, 0], [0xB0, 98, 14], [0xB0, 6, 4]]);
    }
}
