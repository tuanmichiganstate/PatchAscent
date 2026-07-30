//! Opaque-first `SysEx` framing.
//!
//! Manufacturer-specific payloads are not decoded here. Every complete message
//! retains its immutable original bytes and whole-message SHA-256.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const DEFAULT_MAX_SYSEX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysExFramerConfig {
    pub max_message_bytes: usize,
}

impl Default for SysExFramerConfig {
    fn default() -> Self {
        Self {
            max_message_bytes: DEFAULT_MAX_SYSEX_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpaqueSysex {
    original_bytes: Vec<u8>,
    sha256: String,
}

impl OpaqueSysex {
    pub fn from_bytes(bytes: Vec<u8>, max_bytes: usize) -> Result<Self, SysExError> {
        validate_framing(&bytes, max_bytes)?;
        let sha256 = sha256_hex(&bytes);
        Ok(Self {
            original_bytes: bytes,
            sha256,
        })
    }

    pub fn read(path: impl AsRef<Path>, max_bytes: usize) -> Result<Self, SysExFileError> {
        let metadata = fs::metadata(path.as_ref())?;
        let actual = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        if actual > max_bytes {
            return Err(SysExError::MessageTooLarge {
                max: max_bytes,
                actual,
            }
            .into());
        }
        Self::from_bytes(fs::read(path)?, max_bytes).map_err(Into::into)
    }

    pub fn write_byte_identical(&self, path: impl AsRef<Path>) -> Result<(), io::Error> {
        fs::write(path, &self.original_bytes)
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.original_bytes
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[must_use]
    pub fn identity(&self) -> SysexIdentity {
        match self.original_bytes.as_slice() {
            [0xF0, 0x7E, ..] => SysexIdentity::UniversalNonRealtime,
            [0xF0, 0x7F, ..] => SysexIdentity::UniversalRealtime,
            [0xF0, 0x00, first, second, ..] => SysexIdentity::Manufacturer {
                id: vec![0x00, *first, *second],
            },
            [0xF0, manufacturer, ..] if *manufacturer <= 0x7D => SysexIdentity::Manufacturer {
                id: vec![*manufacturer],
            },
            _ => SysexIdentity::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SysexIdentity {
    UniversalNonRealtime,
    UniversalRealtime,
    Manufacturer { id: Vec<u8> },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SysExError {
    #[error("SysEx message is empty")]
    Empty,
    #[error("SysEx message must start with F0")]
    MissingStart,
    #[error("SysEx message must end with F7")]
    MissingEnd,
    #[error("SysEx message exceeds {max} bytes (observed {actual})")]
    MessageTooLarge { max: usize, actual: usize },
    #[error("nested F0 encountered after {partial_len} bytes")]
    NestedStart { partial_len: usize },
    #[error("stray F7 encountered outside a SysEx message")]
    StrayEnd,
    #[error(
        "unexpected status {status:#04x} terminated a SysEx message after {partial_len} bytes"
    )]
    PrematureStatus { status: u8, partial_len: usize },
}

#[derive(Debug, Error)]
pub enum SysExFileError {
    #[error("SysEx file I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Invalid(#[from] SysExError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FramerEvent {
    Complete(OpaqueSysex),
    Diagnostic(SysExError),
}

#[derive(Debug, Clone)]
pub struct SysExFramer {
    config: SysExFramerConfig,
    buffer: Vec<u8>,
}

impl SysExFramer {
    #[must_use]
    pub fn new(config: SysExFramerConfig) -> Self {
        Self {
            config,
            buffer: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_assembling(&self) -> bool {
        !self.buffer.is_empty()
    }

    #[must_use]
    pub fn partial_len(&self) -> usize {
        self.buffer.len()
    }

    /// Consume arbitrary callback chunks. Bytes not enclosed by F0/F7 are left
    /// to the ordinary MIDI decoder and do not appear as `SysEx` events.
    pub fn ingest(&mut self, chunk: &[u8]) -> Vec<FramerEvent> {
        let mut events = Vec::new();
        for &byte in chunk {
            if self.buffer.is_empty() {
                match byte {
                    0xF0 => self.buffer.push(byte),
                    0xF7 => events.push(FramerEvent::Diagnostic(SysExError::StrayEnd)),
                    _ => {}
                }
                continue;
            }

            match byte {
                0xF0 => {
                    events.push(FramerEvent::Diagnostic(SysExError::NestedStart {
                        partial_len: self.buffer.len(),
                    }));
                    self.buffer.clear();
                    self.buffer.push(0xF0);
                }
                0xF7 => {
                    self.buffer.push(byte);
                    let bytes = std::mem::take(&mut self.buffer);
                    match OpaqueSysex::from_bytes(bytes, self.config.max_message_bytes) {
                        Ok(message) => events.push(FramerEvent::Complete(message)),
                        Err(error) => events.push(FramerEvent::Diagnostic(error)),
                    }
                }
                status if (0x80..0xF8).contains(&status) => {
                    events.push(FramerEvent::Diagnostic(SysExError::PrematureStatus {
                        status,
                        partial_len: self.buffer.len(),
                    }));
                    self.buffer.clear();
                }
                _ => {
                    self.buffer.push(byte);
                    if self.buffer.len() > self.config.max_message_bytes {
                        events.push(FramerEvent::Diagnostic(SysExError::MessageTooLarge {
                            max: self.config.max_message_bytes,
                            actual: self.buffer.len(),
                        }));
                        self.buffer.clear();
                    }
                }
            }
        }
        events
    }

    pub fn finish(&mut self) -> Option<SysExError> {
        if self.buffer.is_empty() {
            None
        } else {
            self.buffer.clear();
            Some(SysExError::MissingEnd)
        }
    }
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn validate_framing(bytes: &[u8], max_bytes: usize) -> Result<(), SysExError> {
    if bytes.is_empty() {
        return Err(SysExError::Empty);
    }
    if bytes.len() > max_bytes {
        return Err(SysExError::MessageTooLarge {
            max: max_bytes,
            actual: bytes.len(),
        });
    }
    if bytes.first() != Some(&0xF0) {
        return Err(SysExError::MissingStart);
    }
    if bytes.last() != Some(&0xF7) {
        return Err(SysExError::MissingEnd);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn assembles_across_callbacks_and_hashes_exact_bytes() {
        let mut framer = SysExFramer::new(SysExFramerConfig::default());
        assert!(framer.ingest(&[0xF0, 0x00, 0x20]).is_empty());
        let events = framer.ingest(&[0x29, 0x01, 0xF7]);
        let FramerEvent::Complete(message) = &events[0] else {
            panic!("expected complete message");
        };
        assert_eq!(message.bytes(), &[0xF0, 0x00, 0x20, 0x29, 0x01, 0xF7]);
        assert_eq!(message.sha256().len(), 64);
        assert_eq!(
            message.identity(),
            SysexIdentity::Manufacturer {
                id: vec![0x00, 0x20, 0x29]
            }
        );
    }

    #[test]
    fn opaque_file_round_trip_is_byte_identical() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("input.syx");
        let output = directory.path().join("output.syx");
        let bytes = [0xF0, 0x7E, 0x00, 0x01, 0x7F, 0xF7];
        fs::write(&input, bytes).unwrap();
        let message = OpaqueSysex::read(&input, 1024).unwrap();
        message.write_byte_identical(&output).unwrap();
        assert_eq!(fs::read(output).unwrap(), bytes);
    }

    #[test]
    fn nested_start_is_diagnostic_and_recovers_at_new_start() {
        let mut framer = SysExFramer::new(SysExFramerConfig::default());
        let events = framer.ingest(&[0xF0, 1, 0xF0, 2, 0xF7]);
        assert!(matches!(
            events.as_slice(),
            [
                FramerEvent::Diagnostic(SysExError::NestedStart { partial_len: 2 }),
                FramerEvent::Complete(_)
            ]
        ));
    }

    #[test]
    fn unknown_payload_bytes_survive() {
        let bytes = vec![0xF0, 0x7D, 0, 1, 2, 3, 4, 5, 0xF7];
        let opaque = OpaqueSysex::from_bytes(bytes.clone(), 1024).unwrap();
        assert_eq!(opaque.bytes(), bytes);
    }

    proptest! {
        #[test]
        fn arbitrary_input_never_panics(chunks in prop::collection::vec(any::<u8>(), 0..10_000)) {
            let mut framer = SysExFramer::new(SysExFramerConfig {
                max_message_bytes: 4096,
            });
            let _ = framer.ingest(&chunks);
            let _ = framer.finish();
        }
    }
}
