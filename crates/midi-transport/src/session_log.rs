use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use patchascent_midi_messages::{DecodedMidiMessage, RawMidiEvent};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub application: String,
    pub application_version: String,
    pub peak_os_version: Option<String>,
    pub computer_os: String,
    pub connection: String,
    pub midi_channel: Option<u8>,
    pub cc_nrpn_mode: Option<String>,
    pub bank_patch_mode: Option<String>,
    pub patch_protect: Option<String>,
}

impl SessionMetadata {
    #[must_use]
    pub fn software_only(session_id: Uuid) -> Self {
        Self {
            session_id,
            started_at: Utc::now(),
            application: "peakctl".to_owned(),
            application_version: env!("CARGO_PKG_VERSION").to_owned(),
            peak_os_version: None,
            computer_os: std::env::consts::OS.to_owned(),
            connection: "pending hardware capture".to_owned(),
            midi_channel: None,
            cc_nrpn_mode: None,
            bank_patch_mode: None,
            patch_protect: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record_type", rename_all = "snake_case")]
pub enum SessionRecord {
    SessionStarted {
        metadata: SessionMetadata,
    },
    RawMidi {
        event: RawMidiEvent,
        decoded: DecodedMidiMessage,
    },
    SysexCaptured {
        event_id: u64,
        byte_length: usize,
        sha256: String,
        identity: String,
    },
    Diagnostic {
        timestamp: DateTime<Utc>,
        severity: String,
        message: String,
    },
    SessionFinished {
        timestamp: DateTime<Utc>,
        raw_event_count: u64,
        dropped_event_count: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLogSummary {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub records: u64,
}

#[derive(Debug, Error)]
pub enum SessionLogError {
    #[error("session log I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("session record serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct SessionLogWriter {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    records: u64,
}

impl SessionLogWriter {
    pub fn create(
        directory: impl AsRef<Path>,
        metadata: &SessionMetadata,
    ) -> Result<Self, SessionLogError> {
        fs::create_dir_all(directory.as_ref())?;
        let timestamp = metadata.started_at.format("%Y%m%dT%H%M%S%.3fZ");
        let filename = format!("peakctl-{timestamp}-{}.jsonl", metadata.session_id.simple());
        let path = directory.as_ref().join(filename);
        let writer = BufWriter::new(File::create(&path)?);
        let mut log = Self {
            path,
            writer: Some(writer),
            records: 0,
        };
        log.append(&SessionRecord::SessionStarted {
            metadata: metadata.clone(),
        })?;
        Ok(log)
    }

    pub fn append(&mut self, record: &SessionRecord) -> Result<(), SessionLogError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "session log is finalized"))?;
        serde_json::to_writer(&mut *writer, record)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        self.records += 1;
        Ok(())
    }

    pub fn append_raw(&mut self, event: RawMidiEvent) -> Result<(), SessionLogError> {
        let decoded = event.decoded();
        self.append(&SessionRecord::RawMidi { event, decoded })
    }

    pub fn finalize(mut self) -> Result<SessionLogSummary, SessionLogError> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
            writer.get_ref().sync_all()?;
        }
        let bytes = fs::read(&self.path)?;
        Ok(SessionLogSummary {
            path: self.path,
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
            records: self.records,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use patchascent_midi_messages::MidiDirection;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn creates_timestamped_jsonl_and_hashes_it() {
        let directory = tempdir().unwrap();
        let session_id = Uuid::nil();
        let metadata = SessionMetadata::software_only(session_id);
        let mut log = SessionLogWriter::create(directory.path(), &metadata).unwrap();
        log.append_raw(RawMidiEvent {
            event_id: 1,
            monotonic_timestamp_micros: 2,
            wall_clock_timestamp: Utc::now(),
            port_id: "port".to_owned(),
            port_name: "Peak".to_owned(),
            direction: MidiDirection::Input,
            bytes: vec![0xB0, 79, 64],
            session_id,
        })
        .unwrap();
        let summary = log.finalize().unwrap();
        assert_eq!(summary.records, 2);
        assert_eq!(summary.sha256.len(), 64);
        let contents = fs::read_to_string(summary.path).unwrap();
        assert_eq!(contents.lines().count(), 2);
        assert!(contents.contains("\"record_type\":\"raw_midi\""));
    }
}
