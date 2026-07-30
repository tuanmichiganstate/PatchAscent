use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ParameterId, ParameterScope};

const EMBEDDED_REGISTRY: &str = include_str!("../../../protocol/parameter_registry.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterRegistry {
    pub schema_version: u32,
    pub device_profile: String,
    pub generated_on: String,
    pub policy: RegistryPolicy,
    pub parameters: Vec<ParameterDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the canonical registry policy schema without weakening its explicit gates"
)]
pub struct RegistryPolicy {
    pub exact_os_build: String,
    pub source_defaults_are_executable: bool,
    pub unknown_enum_codes_may_be_guessed: bool,
    pub cc_pair_codec_may_be_assumed_14_bit: bool,
    pub sysex_writes_enabled_by_default: bool,
    pub unknown_sysex_bytes_must_be_preserved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub id: ParameterId,
    pub label: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub section: String,
    pub scope: ParameterScope,
    pub device_scope: String,
    pub binding: Binding,
    pub documented_range: String,
    #[serde(default)]
    pub documented_default: String,
    pub default_policy: String,
    pub display_transform: Option<serde_yaml::Value>,
    pub enum_id: Option<String>,
    pub evidence: EvidenceRecord,
    pub gates: ParameterGates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    Cc { controller: u8 },
    CcPair { controllers: [u8; 2], codec: String },
    Nrpn { msb: u8, lsb: u8 },
    Unmapped,
    Unknown,
}

impl Binding {
    fn key(&self) -> Option<String> {
        match self {
            Self::Cc { controller } => Some(format!("cc:{controller}")),
            Self::CcPair { controllers, .. } => {
                Some(format!("cc_pair:{}:{}", controllers[0], controllers[1]))
            }
            Self::Nrpn { msb, lsb } => Some(format!("nrpn:{msb}:{lsb}")),
            Self::Unmapped | Self::Unknown => None,
        }
    }

    fn validate(&self, parameter_id: &ParameterId) -> Result<(), RegistryError> {
        let validate = |field: &'static str, value: u8| {
            if value <= 0x7F {
                Ok(())
            } else {
                Err(RegistryError::InvalidSevenBitBinding {
                    parameter_id: parameter_id.clone(),
                    field,
                    value,
                })
            }
        };

        match self {
            Self::Cc { controller } => validate("controller", *controller),
            Self::CcPair { controllers, codec } => {
                validate("controllers[0]", controllers[0])?;
                validate("controllers[1]", controllers[1])?;
                if codec.trim().is_empty() {
                    return Err(RegistryError::MissingCcPairCodec(parameter_id.clone()));
                }
                Ok(())
            }
            Self::Nrpn { msb, lsb } => {
                validate("msb", *msb)?;
                validate("lsb", *lsb)
            }
            Self::Unmapped | Self::Unknown => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub status: String,
    pub source_document: String,
    pub source_page: String,
    pub source_row_id: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the canonical per-parameter gate schema"
)]
pub struct ParameterGates {
    pub implementation: String,
    pub live_write_enabled: bool,
    pub live_receive_verified: bool,
    pub sysex_decode_verified: bool,
    pub sysex_encode_verified: bool,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry YAML is invalid: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),
    #[error("duplicate parameter id {0}")]
    DuplicateId(ParameterId),
    #[error("duplicate binding {binding} used by {first} and {second}")]
    DuplicateBinding {
        binding: String,
        first: ParameterId,
        second: ParameterId,
    },
    #[error("{parameter_id}: {field} must be 7-bit, got {value}")]
    InvalidSevenBitBinding {
        parameter_id: ParameterId,
        field: &'static str,
        value: u8,
    },
    #[error("{0}: CC-pair binding must name its evidence-derived codec")]
    MissingCcPairCodec(ParameterId),
    #[error("{0}: the imported seed must keep live writes disabled")]
    SeedWriteEnabled(ParameterId),
    #[error("registry policy must preserve unknown SysEx bytes")]
    UnknownSysexPreservationDisabled,
    #[error("registry policy must not make documentary defaults executable")]
    DocumentaryDefaultsEnabled,
    #[error("registry policy must not guess unknown enum codes")]
    EnumGuessingEnabled,
    #[error("registry policy must not assume conventional 14-bit Peak CC pairs")]
    CcPairAssumptionEnabled,
    #[error("registry policy must not enable SysEx writes by default")]
    SysexWritesEnabled,
}

impl ParameterRegistry {
    pub fn from_yaml(yaml: &str) -> Result<Self, RegistryError> {
        let registry: Self = serde_yaml::from_str(yaml)?;
        registry.validate()?;
        Ok(registry)
    }

    pub fn embedded() -> Result<Self, RegistryError> {
        Self::from_yaml(EMBEDDED_REGISTRY)
    }

    pub fn validate(&self) -> Result<(), RegistryError> {
        if self.policy.source_defaults_are_executable {
            return Err(RegistryError::DocumentaryDefaultsEnabled);
        }
        if self.policy.unknown_enum_codes_may_be_guessed {
            return Err(RegistryError::EnumGuessingEnabled);
        }
        if self.policy.cc_pair_codec_may_be_assumed_14_bit {
            return Err(RegistryError::CcPairAssumptionEnabled);
        }
        if self.policy.sysex_writes_enabled_by_default {
            return Err(RegistryError::SysexWritesEnabled);
        }
        if !self.policy.unknown_sysex_bytes_must_be_preserved {
            return Err(RegistryError::UnknownSysexPreservationDisabled);
        }

        let mut ids = BTreeSet::new();
        let mut bindings = BTreeMap::<String, ParameterId>::new();
        for parameter in &self.parameters {
            if !ids.insert(parameter.id.clone()) {
                return Err(RegistryError::DuplicateId(parameter.id.clone()));
            }
            parameter.binding.validate(&parameter.id)?;
            if let Some(key) = parameter.binding.key() {
                if let Some(first) = bindings.insert(key.clone(), parameter.id.clone()) {
                    return Err(RegistryError::DuplicateBinding {
                        binding: key,
                        first,
                        second: parameter.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn validate_seed_safety(&self) -> Result<(), RegistryError> {
        for parameter in &self.parameters {
            if parameter.gates.live_write_enabled {
                return Err(RegistryError::SeedWriteEnabled(parameter.id.clone()));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn by_id(&self, id: &ParameterId) -> Option<&ParameterDefinition> {
        self.parameters.iter().find(|parameter| &parameter.id == id)
    }

    #[must_use]
    pub fn binding_index(&self) -> BTreeMap<String, &ParameterDefinition> {
        self.parameters
            .iter()
            .filter_map(|parameter| parameter.binding.key().map(|key| (key, parameter)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_is_conservative_and_complete() {
        let registry = ParameterRegistry::embedded().unwrap();
        registry.validate_seed_safety().unwrap();
        assert_eq!(registry.parameters.len(), 251);
        assert!(!registry.policy.source_defaults_are_executable);
        assert!(!registry.policy.cc_pair_codec_may_be_assumed_14_bit);
        assert!(registry.policy.unknown_sysex_bytes_must_be_preserved);
    }

    #[test]
    fn first_cc_test_target_is_present_but_not_enabled() {
        let registry = ParameterRegistry::embedded().unwrap();
        let id = ParameterId::new("filter.filter_resonance").unwrap();
        let parameter = registry.by_id(&id).unwrap();
        assert_eq!(parameter.binding, Binding::Cc { controller: 79 });
        assert!(!parameter.gates.live_write_enabled);
    }

    #[test]
    fn cc_pairs_are_explicitly_unknown() {
        let registry = ParameterRegistry::embedded().unwrap();
        let id = ParameterId::new("filter.filter_frequency").unwrap();
        let parameter = registry.by_id(&id).unwrap();
        assert_eq!(
            parameter.binding,
            Binding::CcPair {
                controllers: [29, 61],
                codec: "unknown_peak_8bit_pair".to_owned(),
            }
        );
        assert!(parameter
            .gates
            .implementation
            .contains("blocked_until_peak_cc_pair"));
    }
}
