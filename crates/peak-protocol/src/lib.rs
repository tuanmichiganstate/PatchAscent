//! Pure Peak protocol codecs.
//!
//! Production builds intentionally have no CC-pair encoder, no manufacturer
//! `SysEx` request, and no stored-memory write command.

mod cc;
mod cc_pair;
mod nrpn;
mod safety;

pub use cc::{encode_filter_resonance_test, FILTER_RESONANCE_CC, FILTER_RESONANCE_PARAMETER_ID};
pub use cc_pair::{CcPairAnalyzer, CcPairCodec, CcPairObservation, PairMember};
#[cfg(feature = "nrpn_candidate_experimental")]
pub use nrpn::{encode_candidate_nrpn, encode_oscillator_1_wave_candidate, NrpnEncodingStrategy};
pub use nrpn::{
    NrpnDiagnosticEvent, NrpnParser, NrpnParserConfig, NrpnSelector, NrpnValue,
    OSCILLATOR_1_WAVE_NRPN,
};
pub use safety::{S1LiveEditAcknowledgement, SafetyClass, SafetyError};
