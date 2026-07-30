use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, TryRecvError, TrySendError};
use midir::{
    Ignore, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};
use patchascent_midi_messages::{MidiDirection, RawMidiEvent};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortDescriptor {
    pub id: String,
    pub name: String,
    pub direction: PortDirection,
    pub backend: String,
    pub occurrence: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortInventory {
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("MIDI backend initialization failed: {0}")]
    Initialization(String),
    #[error("MIDI port metadata failed: {0}")]
    PortInfo(String),
    #[error("MIDI port {direction:?} id {id:?} was not found")]
    PortNotFound {
        direction: PortDirection,
        id: String,
    },
    #[error("MIDI {direction:?} connection to {name:?} failed: {detail}")]
    Connection {
        direction: PortDirection,
        name: String,
        detail: String,
    },
    #[error("MIDI output send failed: {0}")]
    Send(String),
    #[error("input receiver disconnected")]
    InputDisconnected,
}

pub trait MidiBackend: Send + Sync {
    fn list_ports(&self) -> Result<PortInventory, TransportError>;
    fn open_input(
        &self,
        port_id: &str,
        session_id: Uuid,
        capacity: usize,
    ) -> Result<InputSession, TransportError>;
    fn open_output(&self, port_id: &str, session_id: Uuid)
        -> Result<OutputSession, TransportError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MidirBackend;

impl MidiBackend for MidirBackend {
    fn list_ports(&self) -> Result<PortInventory, TransportError> {
        let input = MidiInput::new("PatchAscent port discovery")
            .map_err(|error| TransportError::Initialization(error.to_string()))?;
        let output = MidiOutput::new("PatchAscent port discovery")
            .map_err(|error| TransportError::Initialization(error.to_string()))?;
        Ok(PortInventory {
            inputs: describe_input_ports(&input)?,
            outputs: describe_output_ports(&output)?,
        })
    }

    fn open_input(
        &self,
        port_id: &str,
        session_id: Uuid,
        capacity: usize,
    ) -> Result<InputSession, TransportError> {
        let mut input = MidiInput::new("PatchAscent input")
            .map_err(|error| TransportError::Initialization(error.to_string()))?;
        input.ignore(Ignore::None);
        let (port, descriptor) = find_input_port(&input, port_id)?;
        let (sender, receiver) = bounded(capacity.max(1));
        let dropped_count = Arc::new(AtomicU64::new(0));
        let dropped_for_callback = Arc::clone(&dropped_count);
        let next_event_id = Arc::new(AtomicU64::new(1));
        let ids_for_callback = Arc::clone(&next_event_id);
        let port_id_for_callback = descriptor.id.clone();
        let port_name_for_callback = descriptor.name.clone();
        let connection_name = format!("PatchAscent input {}", session_id.simple());
        let connection = input
            .connect(
                &port,
                &connection_name,
                move |timestamp_micros, bytes, ()| {
                    let event = RawMidiEvent {
                        event_id: ids_for_callback.fetch_add(1, Ordering::Relaxed),
                        monotonic_timestamp_micros: timestamp_micros,
                        wall_clock_timestamp: Utc::now(),
                        port_id: port_id_for_callback.clone(),
                        port_name: port_name_for_callback.clone(),
                        direction: MidiDirection::Input,
                        bytes: bytes.to_vec(),
                        session_id,
                    };
                    if matches!(sender.try_send(event), Err(TrySendError::Full(_))) {
                        dropped_for_callback.fetch_add(1, Ordering::Relaxed);
                    }
                },
                (),
            )
            .map_err(|error| TransportError::Connection {
                direction: PortDirection::Input,
                name: descriptor.name.clone(),
                detail: error.to_string(),
            })?;

        Ok(InputSession {
            descriptor,
            session_id,
            receiver,
            dropped_count,
            connection: Some(connection),
        })
    }

    fn open_output(
        &self,
        port_id: &str,
        session_id: Uuid,
    ) -> Result<OutputSession, TransportError> {
        let output = MidiOutput::new("PatchAscent output")
            .map_err(|error| TransportError::Initialization(error.to_string()))?;
        let (port, descriptor) = find_output_port(&output, port_id)?;
        let connection_name = format!("PatchAscent output {}", session_id.simple());
        let connection = output.connect(&port, &connection_name).map_err(|error| {
            TransportError::Connection {
                direction: PortDirection::Output,
                name: descriptor.name.clone(),
                detail: error.to_string(),
            }
        })?;
        Ok(OutputSession {
            descriptor,
            session_id,
            next_event_id: 1,
            connection: Some(connection),
        })
    }
}

pub struct InputSession {
    descriptor: PortDescriptor,
    session_id: Uuid,
    receiver: Receiver<RawMidiEvent>,
    dropped_count: Arc<AtomicU64>,
    connection: Option<MidiInputConnection<()>>,
}

impl fmt::Debug for InputSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InputSession")
            .field("descriptor", &self.descriptor)
            .field("session_id", &self.session_id)
            .field("dropped_count", &self.dropped_count())
            .field("is_open", &self.connection.is_some())
            .finish_non_exhaustive()
    }
}

impl InputSession {
    #[must_use]
    pub fn descriptor(&self) -> &PortDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn session_id(&self) -> Uuid {
        self.session_id
    }

    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<RawMidiEvent>, TransportError> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(TransportError::InputDisconnected),
        }
    }

    pub fn try_recv(&self) -> Result<Option<RawMidiEvent>, TransportError> {
        match self.receiver.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(TransportError::InputDisconnected),
        }
    }

    pub fn close(mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = connection.close();
        }
    }
}

pub trait MidiOutputSink: Send + 'static {
    fn send(&mut self, bytes: &[u8]) -> Result<RawMidiEvent, TransportError>;
}

pub struct OutputSession {
    descriptor: PortDescriptor,
    session_id: Uuid,
    next_event_id: u64,
    connection: Option<MidiOutputConnection>,
}

impl fmt::Debug for OutputSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutputSession")
            .field("descriptor", &self.descriptor)
            .field("session_id", &self.session_id)
            .field("next_event_id", &self.next_event_id)
            .field("is_open", &self.connection.is_some())
            .finish_non_exhaustive()
    }
}

impl OutputSession {
    #[must_use]
    pub fn descriptor(&self) -> &PortDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn close(mut self) {
        self.connection.take();
    }
}

impl MidiOutputSink for OutputSession {
    fn send(&mut self, bytes: &[u8]) -> Result<RawMidiEvent, TransportError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(TransportError::Send(
                "output connection is already closed".to_owned(),
            ));
        };
        connection
            .send(bytes)
            .map_err(|error| TransportError::Send(error.to_string()))?;
        let event = RawMidiEvent {
            event_id: self.next_event_id,
            monotonic_timestamp_micros: 0,
            wall_clock_timestamp: Utc::now(),
            port_id: self.descriptor.id.clone(),
            port_name: self.descriptor.name.clone(),
            direction: MidiDirection::Output,
            bytes: bytes.to_vec(),
            session_id: self.session_id,
        };
        self.next_event_id += 1;
        Ok(event)
    }
}

fn describe_input_ports(input: &MidiInput) -> Result<Vec<PortDescriptor>, TransportError> {
    let mut occurrences = BTreeMap::<String, usize>::new();
    input
        .ports()
        .iter()
        .map(|port| {
            let name = input
                .port_name(port)
                .map_err(|error| TransportError::PortInfo(error.to_string()))?;
            let occurrence = next_occurrence(&mut occurrences, &name);
            Ok(descriptor(name, PortDirection::Input, occurrence))
        })
        .collect()
}

fn describe_output_ports(output: &MidiOutput) -> Result<Vec<PortDescriptor>, TransportError> {
    let mut occurrences = BTreeMap::<String, usize>::new();
    output
        .ports()
        .iter()
        .map(|port| {
            let name = output
                .port_name(port)
                .map_err(|error| TransportError::PortInfo(error.to_string()))?;
            let occurrence = next_occurrence(&mut occurrences, &name);
            Ok(descriptor(name, PortDirection::Output, occurrence))
        })
        .collect()
}

fn find_input_port(
    input: &MidiInput,
    port_id: &str,
) -> Result<(MidiInputPort, PortDescriptor), TransportError> {
    let mut occurrences = BTreeMap::<String, usize>::new();
    for port in input.ports() {
        let name = input
            .port_name(&port)
            .map_err(|error| TransportError::PortInfo(error.to_string()))?;
        let occurrence = next_occurrence(&mut occurrences, &name);
        let descriptor = descriptor(name, PortDirection::Input, occurrence);
        if descriptor.id == port_id {
            return Ok((port, descriptor));
        }
    }
    Err(TransportError::PortNotFound {
        direction: PortDirection::Input,
        id: port_id.to_owned(),
    })
}

fn find_output_port(
    output: &MidiOutput,
    port_id: &str,
) -> Result<(MidiOutputPort, PortDescriptor), TransportError> {
    let mut occurrences = BTreeMap::<String, usize>::new();
    for port in output.ports() {
        let name = output
            .port_name(&port)
            .map_err(|error| TransportError::PortInfo(error.to_string()))?;
        let occurrence = next_occurrence(&mut occurrences, &name);
        let descriptor = descriptor(name, PortDirection::Output, occurrence);
        if descriptor.id == port_id {
            return Ok((port, descriptor));
        }
    }
    Err(TransportError::PortNotFound {
        direction: PortDirection::Output,
        id: port_id.to_owned(),
    })
}

fn next_occurrence(occurrences: &mut BTreeMap<String, usize>, name: &str) -> usize {
    let next = occurrences.entry(name.to_owned()).or_default();
    let value = *next;
    *next += 1;
    value
}

fn descriptor(name: String, direction: PortDirection, occurrence: usize) -> PortDescriptor {
    let backend = backend_name().to_owned();
    let id = stable_port_id(&backend, direction, &name, occurrence);
    PortDescriptor {
        id,
        name,
        direction,
        backend,
        occurrence,
    }
}

fn stable_port_id(
    backend: &str,
    direction: PortDirection,
    name: &str,
    occurrence: usize,
) -> String {
    let direction = match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
    };
    let source = format!("{backend}\0{direction}\0{name}\0{occurrence}");
    let digest = Sha256::digest(source.as_bytes());
    format!("midi-{}", hex::encode(&digest[..12]))
}

const fn backend_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "core_midi"
    }
    #[cfg(target_os = "windows")]
    {
        "win_mm"
    }
    #[cfg(target_os = "linux")]
    {
        "alsa"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "midir"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_distinguish_direction_and_duplicate_names() {
        let one = stable_port_id("test", PortDirection::Input, "Peak", 0);
        assert_eq!(one, stable_port_id("test", PortDirection::Input, "Peak", 0));
        assert_ne!(
            one,
            stable_port_id("test", PortDirection::Output, "Peak", 0)
        );
        assert_ne!(one, stable_port_id("test", PortDirection::Input, "Peak", 1));
    }
}
