//! Deterministic, software-only Peak simulator for integration tests.
//!
//! The simulator exercises transport failure modes but is never hardware
//! evidence and cannot promote a parameter's verification status.

use std::collections::{BTreeSet, VecDeque};

use patchascent_midi_messages::{decode_message, ChannelMessage, DecodedMidiMessage, MidiChannel};
use patchascent_peak_sysex::{OpaqueSysex, SysExError, DEFAULT_MAX_SYSEX_BYTES};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakePeakConfig {
    pub echo_delay_micros: u64,
    pub accepted_cc_controllers: BTreeSet<u8>,
    pub accepted_nrpn_parameters: BTreeSet<(u8, u8)>,
    pub fixture_sysex: Vec<u8>,
}

impl Default for FakePeakConfig {
    fn default() -> Self {
        Self {
            echo_delay_micros: 0,
            accepted_cc_controllers: BTreeSet::from([79]),
            accepted_nrpn_parameters: BTreeSet::from([(0, 14)]),
            fixture_sysex: vec![0xF0, 0x7D, 0x50, 0x41, 0xF7],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FakePeakMetrics {
    pub received_message_count: u64,
    pub emitted_message_count: u64,
    pub dropped_response_count: u64,
    pub interleaved_nrpn_count: u64,
    pub disconnect_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FakePeakError {
    #[error("fake Peak is disconnected")]
    Disconnected,
    #[error("invalid fixture SysEx: {0}")]
    InvalidFixture(#[from] SysExError),
    #[error("unsupported or malformed MIDI message: {0}")]
    UnsupportedMessage(String),
    #[error("controller {controller} is not accepted by this software-only fixture")]
    UnsupportedController { controller: u8 },
    #[error("NRPN data arrived before a complete selector on channel {channel}")]
    IncompleteNrpn { channel: u8 },
    #[error("NRPN {msb}:{lsb} is not accepted by this software-only fixture")]
    UnsupportedNrpn { msb: u8, lsb: u8 },
    #[error("interleaved NRPN selector detected on channel {channel}")]
    InterleavedNrpn { channel: u8 },
    #[error("program must be a 7-bit value, got {0}")]
    InvalidProgram(u8),
}

#[derive(Debug, Clone, Copy, Default)]
struct NrpnAssembly {
    parameter_msb: Option<u8>,
    parameter_lsb: Option<u8>,
    awaiting_data: bool,
    data_msb_seen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScheduledResponse {
    due_micros: u64,
    order: u64,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct FakePeak {
    config: FakePeakConfig,
    fixture: OpaqueSysex,
    connected: bool,
    now_micros: u64,
    next_order: u64,
    pending: VecDeque<ScheduledResponse>,
    nrpn: [NrpnAssembly; 16],
    drop_next: usize,
    reorder_next_pair: bool,
    metrics: FakePeakMetrics,
}

impl FakePeak {
    pub fn new(config: FakePeakConfig) -> Result<Self, FakePeakError> {
        let fixture =
            OpaqueSysex::from_bytes(config.fixture_sysex.clone(), DEFAULT_MAX_SYSEX_BYTES)?;
        Ok(Self {
            config,
            fixture,
            connected: true,
            now_micros: 0,
            next_order: 1,
            pending: VecDeque::new(),
            nrpn: [NrpnAssembly::default(); 16],
            drop_next: 0,
            reorder_next_pair: false,
            metrics: FakePeakMetrics::default(),
        })
    }

    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    #[must_use]
    pub const fn now_micros(&self) -> u64 {
        self.now_micros
    }

    #[must_use]
    pub const fn metrics(&self) -> FakePeakMetrics {
        self.metrics
    }

    #[must_use]
    pub fn pending_response_count(&self) -> usize {
        self.pending.len()
    }

    pub fn drop_next_responses(&mut self, count: usize) {
        self.drop_next = count;
    }

    pub fn reorder_next_ready_pair(&mut self) {
        self.reorder_next_pair = true;
    }

    pub fn ingest(&mut self, bytes: &[u8]) -> Result<(), FakePeakError> {
        self.require_connected()?;
        let decoded = decode_message(bytes);
        match decoded {
            DecodedMidiMessage::Channel(ChannelMessage::ControlChange {
                channel,
                controller,
                value,
            }) => {
                self.ingest_control_change(channel, controller, value)?;
                self.schedule(bytes.to_vec());
            }
            DecodedMidiMessage::Unknown { reason } => {
                return Err(FakePeakError::UnsupportedMessage(reason));
            }
            other => {
                return Err(FakePeakError::UnsupportedMessage(format!("{other:?}")));
            }
        }
        self.metrics.received_message_count += 1;
        Ok(())
    }

    pub fn emit_program_change(
        &mut self,
        channel: MidiChannel,
        program: u8,
    ) -> Result<(), FakePeakError> {
        self.require_connected()?;
        if program > 0x7F {
            return Err(FakePeakError::InvalidProgram(program));
        }
        self.schedule(vec![0xC0 | channel.zero_based(), program]);
        Ok(())
    }

    pub fn emit_fixture_sysex(&mut self) -> Result<(), FakePeakError> {
        self.require_connected()?;
        self.schedule(self.fixture.bytes().to_vec());
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if self.connected {
            self.metrics.disconnect_count += 1;
        }
        self.connected = false;
        self.pending.clear();
        self.nrpn = [NrpnAssembly::default(); 16];
    }

    pub fn reconnect(&mut self) {
        self.connected = true;
        self.nrpn = [NrpnAssembly::default(); 16];
    }

    #[must_use]
    pub fn advance_by(&mut self, delta_micros: u64) -> Vec<Vec<u8>> {
        self.now_micros = self.now_micros.saturating_add(delta_micros);
        self.take_ready()
    }

    #[must_use]
    pub fn take_ready(&mut self) -> Vec<Vec<u8>> {
        let mut ready = Vec::new();
        while self
            .pending
            .front()
            .is_some_and(|response| response.due_micros <= self.now_micros)
        {
            let response = self
                .pending
                .pop_front()
                .expect("the front response was just checked");
            ready.push(response);
        }
        ready.sort_by_key(|response| (response.due_micros, response.order));
        if self.reorder_next_pair && ready.len() >= 2 {
            ready.swap(0, 1);
            self.reorder_next_pair = false;
        }
        self.metrics.emitted_message_count += ready.len() as u64;
        ready.into_iter().map(|response| response.bytes).collect()
    }

    fn ingest_control_change(
        &mut self,
        channel: MidiChannel,
        controller: u8,
        value: u8,
    ) -> Result<(), FakePeakError> {
        if self.config.accepted_cc_controllers.contains(&controller) {
            return Ok(());
        }
        if matches!(controller, 99 | 98 | 6 | 38) {
            return self.ingest_nrpn(channel, controller, value);
        }
        Err(FakePeakError::UnsupportedController { controller })
    }

    fn ingest_nrpn(
        &mut self,
        channel: MidiChannel,
        controller: u8,
        value: u8,
    ) -> Result<(), FakePeakError> {
        let index = usize::from(channel.zero_based());
        let state = &mut self.nrpn[index];
        match controller {
            99 => {
                if state.awaiting_data {
                    self.metrics.interleaved_nrpn_count += 1;
                    return Err(FakePeakError::InterleavedNrpn {
                        channel: channel.one_based(),
                    });
                }
                *state = NrpnAssembly {
                    parameter_msb: Some(value),
                    ..NrpnAssembly::default()
                };
            }
            98 => {
                state.parameter_lsb = Some(value);
                state.awaiting_data = state.parameter_msb.is_some();
                state.data_msb_seen = false;
            }
            6 => {
                let parameter = selected_parameter(*state, channel)?;
                if !self.config.accepted_nrpn_parameters.contains(&parameter) {
                    return Err(FakePeakError::UnsupportedNrpn {
                        msb: parameter.0,
                        lsb: parameter.1,
                    });
                }
                state.awaiting_data = false;
                state.data_msb_seen = true;
            }
            38 => {
                let parameter = selected_parameter(*state, channel)?;
                if !state.data_msb_seen {
                    return Err(FakePeakError::IncompleteNrpn {
                        channel: channel.one_based(),
                    });
                }
                if !self.config.accepted_nrpn_parameters.contains(&parameter) {
                    return Err(FakePeakError::UnsupportedNrpn {
                        msb: parameter.0,
                        lsb: parameter.1,
                    });
                }
            }
            _ => unreachable!("NRPN controller was filtered by caller"),
        }
        Ok(())
    }

    fn schedule(&mut self, bytes: Vec<u8>) {
        if self.drop_next > 0 {
            self.drop_next -= 1;
            self.metrics.dropped_response_count += 1;
            return;
        }
        let response = ScheduledResponse {
            due_micros: self
                .now_micros
                .saturating_add(self.config.echo_delay_micros),
            order: self.next_order,
            bytes,
        };
        self.next_order = self.next_order.saturating_add(1);
        self.pending.push_back(response);
    }

    fn require_connected(&self) -> Result<(), FakePeakError> {
        if self.connected {
            Ok(())
        } else {
            Err(FakePeakError::Disconnected)
        }
    }
}

fn selected_parameter(
    state: NrpnAssembly,
    channel: MidiChannel,
) -> Result<(u8, u8), FakePeakError> {
    state
        .parameter_msb
        .zip(state.parameter_lsb)
        .ok_or(FakePeakError::IncompleteNrpn {
            channel: channel.one_based(),
        })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use parking_lot::Mutex;
    use patchascent_midi_messages::{MidiDirection, RawMidiEvent};
    use patchascent_midi_transport::{
        MidiOutputSink, OutputScheduler, PacingProfile, TransportError,
    };
    use patchascent_peak_sync::{CommandClass, QueueContext, ScheduledCommand};
    use uuid::Uuid;

    use super::*;

    #[test]
    fn echoes_allowlisted_cc_after_virtual_delay() {
        let mut peak = FakePeak::new(FakePeakConfig {
            echo_delay_micros: 10,
            ..FakePeakConfig::default()
        })
        .unwrap();
        peak.ingest(&[0xB0, 79, 64]).unwrap();
        assert!(peak.take_ready().is_empty());
        assert_eq!(peak.advance_by(9), Vec::<Vec<u8>>::new());
        assert_eq!(peak.advance_by(1), vec![vec![0xB0, 79, 64]]);
    }

    #[test]
    fn injects_drop_and_reorder_faults_deterministically() {
        let mut peak = FakePeak::new(FakePeakConfig::default()).unwrap();
        peak.drop_next_responses(1);
        peak.ingest(&[0xB0, 79, 1]).unwrap();
        peak.ingest(&[0xB0, 79, 2]).unwrap();
        peak.ingest(&[0xB0, 79, 3]).unwrap();
        peak.reorder_next_ready_pair();
        assert_eq!(
            peak.take_ready(),
            vec![vec![0xB0, 79, 3], vec![0xB0, 79, 2]]
        );
        assert_eq!(peak.metrics().dropped_response_count, 1);
    }

    #[test]
    fn emits_program_change_and_exact_fixture_sysex() {
        let mut peak = FakePeak::new(FakePeakConfig::default()).unwrap();
        peak.emit_program_change(MidiChannel::from_one_based(2).unwrap(), 7)
            .unwrap();
        peak.emit_fixture_sysex().unwrap();
        assert_eq!(
            peak.take_ready(),
            vec![vec![0xC1, 7], vec![0xF0, 0x7D, 0x50, 0x41, 0xF7]]
        );
    }

    #[test]
    fn disconnect_cancels_pending_and_rejects_input() {
        let mut peak = FakePeak::new(FakePeakConfig {
            echo_delay_micros: 10,
            ..FakePeakConfig::default()
        })
        .unwrap();
        peak.ingest(&[0xB0, 79, 64]).unwrap();
        peak.disconnect();
        assert_eq!(peak.pending_response_count(), 0);
        assert_eq!(
            peak.ingest(&[0xB0, 79, 64]),
            Err(FakePeakError::Disconnected)
        );
        peak.reconnect();
        peak.ingest(&[0xB0, 79, 65]).unwrap();
    }

    #[test]
    fn detects_interleaved_nrpn_selectors() {
        let mut peak = FakePeak::new(FakePeakConfig::default()).unwrap();
        peak.ingest(&[0xB0, 99, 0]).unwrap();
        peak.ingest(&[0xB0, 98, 14]).unwrap();
        assert_eq!(
            peak.ingest(&[0xB0, 99, 1]),
            Err(FakePeakError::InterleavedNrpn { channel: 1 })
        );
        assert_eq!(peak.metrics().interleaved_nrpn_count, 1);
    }

    #[derive(Debug)]
    struct SimulatorSink {
        peak: Arc<Mutex<FakePeak>>,
        event_id: Arc<AtomicU64>,
    }

    impl MidiOutputSink for SimulatorSink {
        fn send(&mut self, bytes: &[u8]) -> Result<RawMidiEvent, TransportError> {
            self.peak
                .lock()
                .ingest(bytes)
                .map_err(|error| TransportError::Send(error.to_string()))?;
            Ok(RawMidiEvent {
                event_id: self.event_id.fetch_add(1, Ordering::Relaxed),
                monotonic_timestamp_micros: 0,
                wall_clock_timestamp: Utc::now(),
                port_id: "fake-peak-output".to_owned(),
                port_name: "Fake Peak".to_owned(),
                direction: MidiDirection::Output,
                bytes: bytes.to_vec(),
                session_id: Uuid::nil(),
            })
        }
    }

    #[test]
    fn scheduler_keeps_nrpn_atomic_for_fake_peak() {
        let peak = Arc::new(Mutex::new(
            FakePeak::new(FakePeakConfig::default()).unwrap(),
        ));
        let scheduler = OutputScheduler::start(
            SimulatorSink {
                peak: Arc::clone(&peak),
                event_id: Arc::new(AtomicU64::new(1)),
            },
            8,
            PacingProfile {
                between_messages_micros: 0,
            },
        );
        scheduler
            .submit(ScheduledCommand {
                sequence_id: 1,
                context: QueueContext {
                    session_id: Uuid::nil(),
                    patch_epoch: 0,
                },
                class: CommandClass::AtomicSequence,
                parameter_id: None,
                messages: vec![vec![0xB0, 99, 0], vec![0xB0, 98, 14], vec![0xB0, 6, 4]],
                enqueued_at_micros: 0,
            })
            .unwrap();
        scheduler.wait(1, Duration::from_secs(1)).unwrap();

        let mut peak = peak.lock();
        assert_eq!(
            peak.take_ready(),
            vec![vec![0xB0, 99, 0], vec![0xB0, 98, 14], vec![0xB0, 6, 4]]
        );
        assert_eq!(peak.metrics().interleaved_nrpn_count, 0);
    }
}
