//! Local librarian metadata boundary.
//!
//! `SQLite` persistence and hardware memory writes are intentionally deferred
//! until the Milestone 4 evidence gates pass.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeakBank {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareLocation {
    pub bank: PeakBank,
    /// User-facing program number in 1..=128.
    pub program: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPatchMetadata {
    pub object_id: Uuid,
    pub whole_message_sha256: String,
    pub payload_sha256: Option<String>,
    pub user_title: Option<String>,
    pub original_patch_name: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub imported_at: DateTime<Utc>,
    pub hardware_location: Option<HardwareLocation>,
}

impl LocalPatchMetadata {
    #[must_use]
    pub fn opaque(whole_message_sha256: String) -> Self {
        Self {
            object_id: Uuid::new_v4(),
            whole_message_sha256,
            payload_sha256: None,
            user_title: None,
            original_patch_name: None,
            category: None,
            tags: Vec::new(),
            notes: String::new(),
            favorite: false,
            imported_at: Utc::now(),
            hardware_location: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_object_does_not_claim_payload_boundaries() {
        let metadata = LocalPatchMetadata::opaque("0".repeat(64));
        assert_eq!(metadata.payload_sha256, None);
        assert_eq!(metadata.hardware_location, None);
    }
}
