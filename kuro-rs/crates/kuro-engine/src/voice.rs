use std::sync::{Arc, Mutex};

use kuro_core::{Timing, TurnClock, prefer_transcript};
use kuro_router::route_command;
use kuro_tools::brain::{Brain, chat, speakable_line};
use kuro_tts::Speaker;

pub struct VoiceTurn {
    pub timing: Timing,
    pub command_id: String,
    pub speaker: Speaker,
    pub brain: Brain,
}

impl VoiceTurn {
    pub fn new(timing: Timing) -> Self {
        Self { timing, command_id: String::new(), speaker: Speaker::default(), brain: Brain::default() }
    }

    pub fn hotkey_down(&mut self) {
        self.command_id = self.timing.begin("voice");
        self.timing.mark("hotkey_down");
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        live: &str,
        final_text: &str,
        live_match: bool,
        final_match: bool,
        epoch: u64,
        clock: &Arc<Mutex<TurnClock>>,
        llm_mode: &str,
        ollama_up: bool,
        mut on_first_audio: impl FnMut(&str),
    ) -> String {
        self.timing.mark("hotkey_up");
        let text = prefer_transcript(live, final_text, live_match, final_match);
        self.timing.mark("stt_done");
        let live_now = clock.lock().map(|c| c.live(epoch)).unwrap_or(false);
        if text.trim().is_empty() || !live_now {
            self.timing.mark("idle");
            let id = self.command_id.clone();
            self.timing.finish(&id);
            return String::new();
        }
        let mut routed = route_command(&text, live_now, None, llm_mode, ollama_up);
        if routed.reply.is_empty() && routed.start_ollama && ollama_up && live_now {
            let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:7b-instruct".into());
            let messages = self.brain.thread(&text).to_vec();
            match chat(&model, &messages) {
                Ok(answer) => {
                    let remembered = self.brain.remember(&answer);
                    routed.reply = remembered;
                }
                Err(err) => {
                    routed.reply = err;
                }
            }
        } else if routed.reply.is_empty() && routed.start_ollama && !ollama_up {
            routed.reply = "Neural engine is off. Please wait while I start it.".into();
        }
        self.timing.mark("route_done");
        let still_live = clock.lock().map(|c| c.live(epoch)).unwrap_or(false);
        if !routed.reply.is_empty() && !routed.skip_tts && still_live {
            let spoken = speakable_line(&routed.reply);
            let spoken_hud = spoken.clone();
            let clock = Arc::clone(clock);
            let VoiceTurn { timing, speaker, .. } = self;
            speaker.speak(
                &spoken,
                move || clock.lock().map(|c| c.live(epoch)).unwrap_or(false),
                || {
                    timing.once("tts", "tts_first_audio", 0.0);
                    on_first_audio(&spoken_hud);
                },
            );
        }
        self.timing.mark("idle");
        let id = self.command_id.clone();
        self.timing.finish(&id);
        routed.reply
    }
}
