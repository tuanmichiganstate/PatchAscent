use patchascent_peak_domain::{
    ChangeSource, GlobalSettingsState, ParameterChange, ParameterDefinition, ParameterScope,
    ParameterValue, PatchState, ValueProvenance,
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditorState {
    pub patch: PatchState,
    pub global_settings: GlobalSettingsState,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReducerError {
    #[error("runtime-clock and unknown parameters do not belong to patch/global state")]
    UnsupportedScope,
    #[error("change id {change_id} does not match registry definition {definition_id}")]
    ParameterMismatch {
        change_id: String,
        definition_id: String,
    },
}

impl EditorState {
    pub fn apply(
        &mut self,
        definition: &ParameterDefinition,
        change: ParameterChange,
        session_id: Uuid,
    ) -> Result<(), ReducerError> {
        let change = change.normalized();
        if change.parameter_id != definition.id {
            return Err(ReducerError::ParameterMismatch {
                change_id: change.parameter_id.to_string(),
                definition_id: definition.id.to_string(),
            });
        }
        let provenance = match change.source {
            ChangeSource::PeakHardware => ValueProvenance::HardwarePanel {
                event_id: change.event_id,
            },
            ChangeSource::SysexLoad => ValueProvenance::SysexCapture {
                sha256: "pending-capture-correlation".to_owned(),
            },
            ChangeSource::ProgramSelection => ValueProvenance::ProgramSelection,
            ChangeSource::Initialization => ValueProvenance::InitializationFixture {
                fixture_id: "explicit-initialization".to_owned(),
            },
            ChangeSource::ProtocolLab => ValueProvenance::ProtocolLab {
                session_id,
                event_id: change.event_id,
            },
            ChangeSource::UserInterface | ChangeSource::Undo | ChangeSource::Redo => {
                ValueProvenance::UserIntent {
                    event_id: change.event_id,
                }
            }
        };
        let value = ParameterValue::Known {
            raw: change.new_raw,
            provenance,
            verification: change.verification,
        };
        match definition.scope {
            ParameterScope::Patch => {
                self.patch.values.insert(change.parameter_id, value);
                if !matches!(
                    change.source,
                    ChangeSource::Initialization
                        | ChangeSource::SysexLoad
                        | ChangeSource::ProgramSelection
                ) {
                    self.patch.dirty = true;
                }
            }
            ParameterScope::Global => {
                self.global_settings
                    .values
                    .insert(change.parameter_id, value);
            }
            ParameterScope::RuntimeClock | ParameterScope::Unknown => {
                return Err(ReducerError::UnsupportedScope);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use patchascent_peak_domain::{ParameterId, ParameterRegistry, VerificationStatus};

    use super::*;

    fn change(parameter_id: &str, value: i32) -> ParameterChange {
        ParameterChange {
            event_id: 1,
            parameter_id: ParameterId::new(parameter_id).unwrap(),
            old_raw: None,
            new_raw: value,
            source: ChangeSource::PeakHardware,
            request_hardware_send: true,
            timestamp_micros: 1,
            verification: VerificationStatus::ReceiveVerified,
        }
    }

    #[test]
    fn global_change_never_marks_patch_dirty() {
        let registry = ParameterRegistry::embedded().unwrap();
        let id = ParameterId::new("settings.patch_cue").unwrap();
        let definition = registry.by_id(&id).unwrap();
        let mut state = EditorState::default();
        state
            .apply(definition, change(id.as_str(), 1), Uuid::nil())
            .unwrap();
        assert!(!state.patch.dirty);
        assert!(state.global_settings.values.contains_key(&id));
    }

    #[test]
    fn hardware_patch_edit_is_stored_without_echo_intent() {
        let registry = ParameterRegistry::embedded().unwrap();
        let id = ParameterId::new("filter.filter_resonance").unwrap();
        let definition = registry.by_id(&id).unwrap();
        let mut state = EditorState::default();
        state
            .apply(definition, change(id.as_str(), 64), Uuid::nil())
            .unwrap();
        assert!(state.patch.dirty);
        assert!(state.patch.values.contains_key(&id));
    }
}
