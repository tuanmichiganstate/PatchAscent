//! Synchronization primitives independent of MIDI backends and React.

mod correlation;
mod queue;
mod reducer;

pub use correlation::{
    CorrelationOutcome, CorrelationTracker, InboundSemanticValue, OutboundCorrelation,
};
pub use queue::{
    CommandClass, CommandQueue, QueueContext, QueueError, QueueMetrics, ScheduledCommand,
};
pub use reducer::{EditorState, ReducerError};
