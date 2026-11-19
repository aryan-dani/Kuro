//! Execute `kuro_turn` effects on the voice thread (TTS, speak generation cancel).

use std::sync::{Arc, Mutex};

use crate::turn_driver::RunContext;
use kuro_core::TurnClock;
use kuro_tools::brain::speakable_line;
use kuro_turn::{Effect, Event, State};

pub(crate) fn apply_turn_effects(
    ctx: &mut RunContext<'_>,
    effects: Vec<Effect>,
    epoch: u64,
    clock: &Arc<Mutex<TurnClock>>,
    on_first_audio: &mut impl FnMut(&str),
) {
    for effect in effects {
        match effect {
            Effect::Speak { text } => {
                let speak_gen = clock.lock().map(|c| c.speak_gen).unwrap_or(0);
                let still_live = clock.lock().map(|c| c.speak_live(epoch, speak_gen)).unwrap_or(false);
                if still_live {
                    let spoken = speakable_line(&text);
                    let spoken_hud = spoken.clone();
                    let clock = Arc::clone(clock);
                    let timing = &mut *ctx.timing;
                    let played = ctx.speaker.speak(
                        &spoken,
                        move || clock.lock().map(|c| c.speak_live(epoch, speak_gen)).unwrap_or(false),
                        || {
                            timing.once("tts", "tts_first_audio", 0.0);
                            on_first_audio(&spoken_hud);
                        },
                    );
                    if !played {
                        let preview: String = spoken.chars().take(96).collect();
                        crate::util::log_line(&format!("tts skipped or empty synth: `{preview}`"));
                        // Still surface the reply in the HUD even when audio fails.
                        on_first_audio(&spoken);
                    }
                }
                if matches!(ctx.runner.state(), State::Speaking) {
                    let _ = ctx.runner.on_event(Event::SpeakDone);
                }
            }
            Effect::CancelSpeak => {
                if let Ok(mut c) = clock.lock() {
                    c.cancel_speak();
                }
            }
            Effect::StartMic
            | Effect::StopMic
            | Effect::Transcribe
            | Effect::Route
            | Effect::Execute
            | Effect::PromptConfirm { .. }
            | Effect::ExecuteConfirmed { .. }
            | Effect::SetHud(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use kuro_core::{Timing, TurnClock};
    use kuro_tools::brain::Brain;
    use kuro_turn::{Effect, TurnRunner};

    use kuro_tts::Speaker;

    use super::*;
    use crate::turn_agent::AgentHook;
    use crate::turn_driver::RunContext;

    #[test]
    fn cancel_speak_effect_bumps_speak_generation() {
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let epoch = clock.lock().unwrap().begin_turn();
        let gen0 = clock.lock().unwrap().speak_gen;
        let dir = std::env::temp_dir().join(format!("kuro-cancel-speak-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("timing.csv");
        let mut runner = TurnRunner::new();
        let mut timing = Timing::new(&path, false);
        let mut speaker = Speaker::default();
        let mut brain = Brain::default();
        let agent = AgentHook::default();
        let mut ctx = RunContext {
            timing: &mut timing,
            command_id: "t",
            speaker: &mut speaker,
            brain: &mut brain,
            runner: &mut runner,
            agent: &agent,
        };
        apply_turn_effects(&mut ctx, vec![Effect::CancelSpeak], epoch, &clock, &mut |_| {});
        assert_eq!(clock.lock().unwrap().speak_gen, gen0 + 1);
        assert!(!clock.lock().unwrap().speak_live(epoch, gen0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
