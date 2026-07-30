//! Evidence-aware domain state for `PatchAscent`.

mod registry;
mod state;

pub use registry::{
    Binding, EvidenceRecord, ParameterDefinition, ParameterGates, ParameterRegistry, RegistryError,
    RegistryPolicy,
};
pub use state::{
    ChangeSource, ConnectionState, DeviceSessionState, EditorHistoryState, GlobalSettingsState,
    LibrarianState, ParameterChange, ParameterId, ParameterScope, ParameterValue, PatchState,
    ValueProvenance, VerificationStatus,
};
