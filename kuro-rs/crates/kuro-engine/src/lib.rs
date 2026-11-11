mod voice;

pub use voice::VoiceTurn;

pub fn voice_turn(timing: kuro_core::Timing) -> VoiceTurn {
    VoiceTurn::new(timing)
}
