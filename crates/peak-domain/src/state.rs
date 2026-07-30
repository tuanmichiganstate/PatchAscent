use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParameterId(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParameterIdError {
    #[error("parameter id must contain at least one namespace separator")]
    MissingNamespace,
    #[error("parameter id contains invalid character {0:?}")]
    InvalidCharacter(char),
}

impl ParameterId {
    pub fn new(value: impl Into<String>) -> Result<Self, ParameterIdError> {
        let value = value.into();
        if !value.contains('.') {
            return Err(ParameterIdError::MissingNamespace);
        }
        if let Some(character) = value.chars().find(|character| {
            !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '.'))
        }) {
            return Err(ParameterIdError::InvalidCharacter(character));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ParameterId {
    type Err = ParameterIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for ParameterId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ParameterId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterScope {
    Patch,
    Global,
    RuntimeClock,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSource {
    UserInterface,
    PeakHardware,
    SysexLoad,
    ProgramSelection,
    Undo,
    Redo,
    Initialization,
    ProtocolLab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    DocumentedUnverified,
    DocumentConflict,
    ReceiveVerified,
    SendVerified,
    SemanticVerified,
    SysexDecodeVerified,
    SysexRoundTripVerified,
    MemoryWriteVerified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ValueProvenance {
    HardwarePanel { event_id: u64 },
    UserIntent { event_id: u64 },
    SysexCapture { sha256: String },
    ProgramSelection,
    InitializationFixture { fixture_id: String },
    ProtocolLab { session_id: Uuid, event_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ParameterValue {
    #[default]
    Unknown,
    Known {
        raw: i32,
        provenance: ValueProvenance,
        verification: VerificationStatus,
    },
    Conflicted {
        candidates: Vec<i32>,
        provenance: ValueProvenance,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterChange {
    pub event_id: u64,
    pub parameter_id: ParameterId,
    pub old_raw: Option<i32>,
    pub new_raw: i32,
    pub source: ChangeSource,
    pub request_hardware_send: bool,
    pub timestamp_micros: u64,
    pub verification: VerificationStatus,
}

impl ParameterChange {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if matches!(self.source, ChangeSource::PeakHardware) {
            self.request_hardware_send = false;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PatchState {
    pub values: BTreeMap<ParameterId, ParameterValue>,
    pub dirty: bool,
    pub baseline_capture_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GlobalSettingsState {
    pub values: BTreeMap<ParameterId, ParameterValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    ConnectedUnidentified,
    Identified,
    Synchronizing,
    Ready,
    Degraded,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSessionState {
    pub session_id: Uuid,
    pub connection: ConnectionState,
    pub input_port_id: Option<String>,
    pub output_port_id: Option<String>,
    pub midi_channel: u8,
    pub exact_peak_os_build: Option<String>,
    pub incomplete_patch_state: bool,
}

impl Default for DeviceSessionState {
    fn default() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            connection: ConnectionState::Disconnected,
            input_port_id: None,
            output_port_id: None,
            midi_channel: 1,
            exact_peak_os_build: None,
            incomplete_patch_state: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LibrarianState {
    pub selected_local_object_id: Option<Uuid>,
    pub whole_message_hashes: BTreeMap<Uuid, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EditorHistoryState {
    pub undo: Vec<ParameterChange>,
    pub redo: Vec<ParameterChange>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_not_coerced_to_zero() {
        let serialized = serde_yaml::to_string(&ParameterValue::Unknown).unwrap();
        assert!(serialized.contains("unknown"));
        assert!(!serialized.contains("raw"));
    }

    #[test]
    fn hardware_changes_cannot_request_echo_send() {
        let change = ParameterChange {
            event_id: 1,
            parameter_id: ParameterId::new("filter.filter_resonance").unwrap(),
            old_raw: None,
            new_raw: 64,
            source: ChangeSource::PeakHardware,
            request_hardware_send: true,
            timestamp_micros: 1,
            verification: VerificationStatus::ReceiveVerified,
        }
        .normalized();
        assert!(!change.request_hardware_send);
    }

    #[test]
    fn deserialization_cannot_bypass_parameter_id_validation() {
        assert!(serde_yaml::from_str::<ParameterId>("invalid").is_err());
        assert_eq!(
            serde_yaml::from_str::<ParameterId>("filter.filter_resonance")
                .unwrap()
                .as_str(),
            "filter.filter_resonance"
        );
    }
}
