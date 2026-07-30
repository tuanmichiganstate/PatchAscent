use patchascent_midi_messages::{encode_control_change, MidiChannel, MidiMessageError};

use crate::S1LiveEditAcknowledgement;

pub const FILTER_RESONANCE_PARAMETER_ID: &str = "filter.filter_resonance";
pub const FILTER_RESONANCE_CC: u8 = 79;

/// The only initially allowlisted CC write.
///
/// This changes only the current edit buffer. It is still audible and therefore
/// requires an explicit S1 acknowledgement at the command boundary.
pub fn encode_filter_resonance_test(
    channel: MidiChannel,
    value: u8,
    _acknowledgement: S1LiveEditAcknowledgement,
) -> Result<[u8; 3], MidiMessageError> {
    encode_control_change(channel, FILTER_RESONANCE_CC, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_allowlisted_cc_bytes() {
        let channel = MidiChannel::from_one_based(16).unwrap();
        let acknowledgement = S1LiveEditAcknowledgement::from_cli_flag(true).unwrap();
        assert_eq!(
            encode_filter_resonance_test(channel, 127, acknowledgement).unwrap(),
            [0xBF, 79, 127]
        );
    }
}
