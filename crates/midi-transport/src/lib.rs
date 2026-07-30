//! MIDI transport boundary for `PatchAscent`.

mod midir_backend;
mod scheduler;
mod session_log;

pub use midir_backend::{
    InputSession, MidiBackend, MidiOutputSink, MidirBackend, OutputSession, PortDescriptor,
    PortDirection, PortInventory, TransportError,
};
pub use scheduler::{
    OutputScheduler, PacingProfile, SchedulerError, SchedulerStatus, SubmissionReceipt,
};
pub use session_log::{SessionLogSummary, SessionLogWriter, SessionMetadata, SessionRecord};
