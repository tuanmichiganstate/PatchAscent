use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyClass {
    PassiveMonitoring,
    LiveEditBufferChange,
    TemporarySysexTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct S1LiveEditAcknowledgement(());

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SafetyError {
    #[error(
        "live MIDI edits require explicit acknowledgement that the current Peak edit buffer and sound may change"
    )]
    MissingLiveEditAcknowledgement,
}

impl S1LiveEditAcknowledgement {
    pub fn from_cli_flag(acknowledged: bool) -> Result<Self, SafetyError> {
        if acknowledged {
            Ok(Self(()))
        } else {
            Err(SafetyError::MissingLiveEditAcknowledgement)
        }
    }
}
