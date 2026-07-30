//! Byte-preserving MIDI 1.0 event and channel-message primitives.
//!
//! Decoding is always derived from a [`RawMidiEvent`]. Recognition never replaces
//! or mutates the original bytes.

use chrono::{DateTime, Utc};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use uuid::Uuid;

/// Direction of a raw event relative to `PatchAscent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiDirection {
    Input,
    Output,
}

/// A MIDI callback or outgoing message exactly as observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMidiEvent {
    pub event_id: u64,
    pub monotonic_timestamp_micros: u64,
    pub wall_clock_timestamp: DateTime<Utc>,
    pub port_id: String,
    pub port_name: String,
    pub direction: MidiDirection,
    pub bytes: Vec<u8>,
    pub session_id: Uuid,
}

impl RawMidiEvent {
    #[must_use]
    pub fn decoded(&self) -> DecodedMidiMessage {
        decode_message(&self.bytes)
    }
}

/// A validated MIDI channel in the user-facing range 1 through 16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MidiChannel(u8);

impl MidiChannel {
    pub const MIN_ONE_BASED: u8 = 1;
    pub const MAX_ONE_BASED: u8 = 16;

    pub fn from_one_based(value: u8) -> Result<Self, MidiMessageError> {
        if (Self::MIN_ONE_BASED..=Self::MAX_ONE_BASED).contains(&value) {
            Ok(Self(value - 1))
        } else {
            Err(MidiMessageError::InvalidChannel(value))
        }
    }

    pub fn from_zero_based(value: u8) -> Result<Self, MidiMessageError> {
        if value < 16 {
            Ok(Self(value))
        } else {
            Err(MidiMessageError::InvalidChannel(value.saturating_add(1)))
        }
    }

    #[must_use]
    pub const fn one_based(self) -> u8 {
        self.0 + 1
    }

    #[must_use]
    pub const fn zero_based(self) -> u8 {
        self.0
    }
}

impl Serialize for MidiChannel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.one_based())
    }
}

impl<'de> Deserialize<'de> for MidiChannel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Self::from_one_based(value).map_err(D::Error::custom)
    }
}

/// Standard MIDI 1.0 channel messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChannelMessage {
    NoteOff {
        channel: MidiChannel,
        note: u8,
        velocity: u8,
    },
    NoteOn {
        channel: MidiChannel,
        note: u8,
        velocity: u8,
    },
    PolyphonicKeyPressure {
        channel: MidiChannel,
        note: u8,
        pressure: u8,
    },
    ControlChange {
        channel: MidiChannel,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: MidiChannel,
        program: u8,
    },
    ChannelPressure {
        channel: MidiChannel,
        pressure: u8,
    },
    PitchBend {
        channel: MidiChannel,
        value_14bit: u16,
    },
}

impl ChannelMessage {
    #[must_use]
    pub const fn channel(&self) -> MidiChannel {
        match self {
            Self::NoteOff { channel, .. }
            | Self::NoteOn { channel, .. }
            | Self::PolyphonicKeyPressure { channel, .. }
            | Self::ControlChange { channel, .. }
            | Self::ProgramChange { channel, .. }
            | Self::ChannelPressure { channel, .. }
            | Self::PitchBend { channel, .. } => *channel,
        }
    }
}

/// Recognized non-channel messages useful in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemMessage {
    SystemExclusive,
    MidiTimeCodeQuarterFrame { value: u8 },
    SongPositionPointer { value_14bit: u16 },
    SongSelect { song: u8 },
    TuneRequest,
    EndOfExclusive,
    TimingClock,
    Start,
    Continue,
    Stop,
    ActiveSensing,
    Reset,
}

/// Best-effort interpretation that leaves unsupported or malformed bytes explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum DecodedMidiMessage {
    Channel(ChannelMessage),
    System(SystemMessage),
    Unknown { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MidiMessageError {
    #[error("MIDI channel must be in 1..=16, got {0}")]
    InvalidChannel(u8),
    #[error("{field} must be a 7-bit value, got {value}")]
    InvalidSevenBitValue { field: &'static str, value: u8 },
    #[error("message has status {status:#04x} but length {actual}; expected {expected}")]
    InvalidLength {
        status: u8,
        expected: usize,
        actual: usize,
    },
    #[error("first byte {0:#04x} is not a MIDI status byte")]
    MissingStatus(u8),
}

/// Encode an ordinary three-byte Control Change message.
pub fn encode_control_change(
    channel: MidiChannel,
    controller: u8,
    value: u8,
) -> Result<[u8; 3], MidiMessageError> {
    require_seven_bit("controller", controller)?;
    require_seven_bit("value", value)?;
    Ok([0xB0 | channel.zero_based(), controller, value])
}

/// Decode one complete backend-delivered MIDI message.
#[must_use]
pub fn decode_message(bytes: &[u8]) -> DecodedMidiMessage {
    match decode_message_result(bytes) {
        Ok(message) => message,
        Err(error) => DecodedMidiMessage::Unknown {
            reason: error.to_string(),
        },
    }
}

fn decode_message_result(bytes: &[u8]) -> Result<DecodedMidiMessage, MidiMessageError> {
    let Some(&status) = bytes.first() else {
        return Ok(DecodedMidiMessage::Unknown {
            reason: "empty MIDI callback".to_owned(),
        });
    };
    if status < 0x80 {
        return Err(MidiMessageError::MissingStatus(status));
    }

    if status < 0xF0 {
        return decode_channel_message(bytes).map(DecodedMidiMessage::Channel);
    }

    let system = match status {
        0xF0 if bytes.last() == Some(&0xF7) => SystemMessage::SystemExclusive,
        0xF0 => {
            return Ok(DecodedMidiMessage::Unknown {
                reason: "unterminated System Exclusive fragment".to_owned(),
            });
        }
        0xF1 => {
            expect_length(bytes, 2)?;
            require_seven_bit("quarter frame", bytes[1])?;
            SystemMessage::MidiTimeCodeQuarterFrame { value: bytes[1] }
        }
        0xF2 => {
            expect_length(bytes, 3)?;
            validate_data_bytes(&bytes[1..])?;
            SystemMessage::SongPositionPointer {
                value_14bit: u16::from(bytes[1]) | (u16::from(bytes[2]) << 7),
            }
        }
        0xF3 => {
            expect_length(bytes, 2)?;
            require_seven_bit("song", bytes[1])?;
            SystemMessage::SongSelect { song: bytes[1] }
        }
        0xF6 => {
            expect_length(bytes, 1)?;
            SystemMessage::TuneRequest
        }
        0xF7 => {
            expect_length(bytes, 1)?;
            SystemMessage::EndOfExclusive
        }
        0xF8 => {
            expect_length(bytes, 1)?;
            SystemMessage::TimingClock
        }
        0xFA => {
            expect_length(bytes, 1)?;
            SystemMessage::Start
        }
        0xFB => {
            expect_length(bytes, 1)?;
            SystemMessage::Continue
        }
        0xFC => {
            expect_length(bytes, 1)?;
            SystemMessage::Stop
        }
        0xFE => {
            expect_length(bytes, 1)?;
            SystemMessage::ActiveSensing
        }
        0xFF => {
            expect_length(bytes, 1)?;
            SystemMessage::Reset
        }
        _ => {
            return Ok(DecodedMidiMessage::Unknown {
                reason: format!("unsupported system status {status:#04x}"),
            });
        }
    };
    Ok(DecodedMidiMessage::System(system))
}

fn decode_channel_message(bytes: &[u8]) -> Result<ChannelMessage, MidiMessageError> {
    let status = bytes[0];
    let family = status & 0xF0;
    let channel = MidiChannel::from_zero_based(status & 0x0F)?;
    let expected = if matches!(family, 0xC0 | 0xD0) { 2 } else { 3 };
    expect_length(bytes, expected)?;
    validate_data_bytes(&bytes[1..])?;

    Ok(match family {
        0x80 => ChannelMessage::NoteOff {
            channel,
            note: bytes[1],
            velocity: bytes[2],
        },
        0x90 => ChannelMessage::NoteOn {
            channel,
            note: bytes[1],
            velocity: bytes[2],
        },
        0xA0 => ChannelMessage::PolyphonicKeyPressure {
            channel,
            note: bytes[1],
            pressure: bytes[2],
        },
        0xB0 => ChannelMessage::ControlChange {
            channel,
            controller: bytes[1],
            value: bytes[2],
        },
        0xC0 => ChannelMessage::ProgramChange {
            channel,
            program: bytes[1],
        },
        0xD0 => ChannelMessage::ChannelPressure {
            channel,
            pressure: bytes[1],
        },
        0xE0 => ChannelMessage::PitchBend {
            channel,
            value_14bit: u16::from(bytes[1]) | (u16::from(bytes[2]) << 7),
        },
        _ => unreachable!("channel family was bounded by status range"),
    })
}

fn expect_length(bytes: &[u8], expected: usize) -> Result<(), MidiMessageError> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(MidiMessageError::InvalidLength {
            status: bytes[0],
            expected,
            actual: bytes.len(),
        })
    }
}

fn validate_data_bytes(bytes: &[u8]) -> Result<(), MidiMessageError> {
    for &value in bytes {
        require_seven_bit("data", value)?;
    }
    Ok(())
}

fn require_seven_bit(field: &'static str, value: u8) -> Result<(), MidiMessageError> {
    if value <= 0x7F {
        Ok(())
    } else {
        Err(MidiMessageError::InvalidSevenBitValue { field, value })
    }
}

/// Format bytes for the human-facing raw monitor.
#[must_use]
pub fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reassembles channel messages when a lower-level backend exposes running status.
#[derive(Debug, Default)]
pub struct RunningStatusDecoder {
    running_status: Option<u8>,
    pending: Vec<u8>,
}

impl RunningStatusDecoder {
    #[must_use]
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut messages = Vec::new();
        for &byte in bytes {
            if byte >= 0xF8 {
                messages.push(vec![byte]);
                continue;
            }
            if byte >= 0x80 {
                self.pending.clear();
                if byte < 0xF0 {
                    self.running_status = Some(byte);
                    self.pending.push(byte);
                } else {
                    self.running_status = None;
                    messages.push(vec![byte]);
                }
                continue;
            }

            if self.pending.is_empty() {
                if let Some(status) = self.running_status {
                    self.pending.push(status);
                } else {
                    messages.push(vec![byte]);
                    continue;
                }
            }
            self.pending.push(byte);
            if self.pending.len() == channel_message_length(self.pending[0]) {
                messages.push(self.pending.clone());
                let status = self.pending[0];
                self.pending.clear();
                self.pending.push(status);
            }
        }
        messages
    }
}

const fn channel_message_length(status: u8) -> usize {
    if matches!(status & 0xF0, 0xC0 | 0xD0) {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn decodes_every_standard_channel_family() {
        let cases = [
            (vec![0x80, 60, 1], "note_off"),
            (vec![0x90, 60, 2], "note_on"),
            (vec![0xA0, 60, 3], "polyphonic_key_pressure"),
            (vec![0xB0, 79, 4], "control_change"),
            (vec![0xC0, 5], "program_change"),
            (vec![0xD0, 6], "channel_pressure"),
            (vec![0xE0, 0, 64], "pitch_bend"),
        ];
        for (bytes, expected_type) in cases {
            let json = serde_json::to_value(decode_message(&bytes)).unwrap();
            assert_eq!(json["kind"], "channel");
            assert_eq!(json["message"]["type"], expected_type);
        }
    }

    #[test]
    fn preserves_running_status_and_realtime_messages() {
        let mut decoder = RunningStatusDecoder::default();
        let messages = decoder.push(&[0xB2, 79, 1, 79, 2, 0xF8, 79, 3]);
        assert_eq!(
            messages,
            vec![
                vec![0xB2, 79, 1],
                vec![0xB2, 79, 2],
                vec![0xF8],
                vec![0xB2, 79, 3]
            ]
        );
    }

    #[test]
    fn rejects_non_data_values() {
        let channel = MidiChannel::from_one_based(1).unwrap();
        assert!(encode_control_change(channel, 128, 0).is_err());
        assert!(encode_control_change(channel, 79, 128).is_err());
    }

    #[test]
    fn channel_json_is_one_based_and_validated() {
        let channel = MidiChannel::from_one_based(16).unwrap();
        assert_eq!(serde_json::to_string(&channel).unwrap(), "16");
        assert_eq!(
            serde_json::from_str::<MidiChannel>("1")
                .unwrap()
                .one_based(),
            1
        );
        assert!(serde_json::from_str::<MidiChannel>("0").is_err());
        assert!(serde_json::from_str::<MidiChannel>("17").is_err());
    }

    proptest! {
        #[test]
        fn cc_round_trip_is_exact(
            channel in 1_u8..=16,
            controller in 0_u8..=127,
            value in 0_u8..=127
        ) {
            let channel = MidiChannel::from_one_based(channel).unwrap();
            let bytes = encode_control_change(channel, controller, value).unwrap();
            prop_assert_eq!(
                decode_message(&bytes),
                DecodedMidiMessage::Channel(ChannelMessage::ControlChange {
                    channel,
                    controller,
                    value,
                })
            );
        }
    }
}
