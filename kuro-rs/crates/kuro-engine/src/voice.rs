use std::sync::{Arc, Mutex};

use kuro_core::{Timing, TurnClock};
use kuro_tools::brain::Brain;
use kuro_tts::Speaker;
use kuro_turn::{State, TurnRunner};

use crate::turn_agent::AgentHook;
use crate::turn_driver::{PostHotkeyInput, RunContext, run_post_hotkey_up};
use crate::turn_effects::apply_turn_effects;

pub struct VoiceTurn {
    pub timing: Timing,
    pub command_id: String,
    pub speaker: Speaker,
    pub brain: Brain,
    runner: TurnRunner,
    agent: AgentHook,
}

impl VoiceTurn {
    pub fn new(timing: Timing) -> Self {
        Self::with_agent(timing, AgentHook::default())
    }

    pub fn with_agent(timing: Timing, agent: AgentHook) -> Self {
        Self {
            timing,
            command_id: String::new(),
            speaker: Speaker::default(),
            brain: Brain::default(),
            runner: TurnRunner::new(),
            agent,
        }
    }

    #[cfg(test)]
    pub(crate) fn runner(&mut self) -> &mut TurnRunner {
        &mut self.runner
    }

    pub fn hotkey_down(&mut self, clock: &Arc<Mutex<TurnClock>>) {
        self.command_id = self.timing.begin("voice");
        self.timing.mark("hotkey_down");
        let epoch = clock.lock().map(|c| c.snapshot()).unwrap_or(0);
        let effects = self.runner.on_event(kuro_turn::Event::HotkeyDown);
        let mut ctx = RunContext {
            timing: &mut self.timing,
            command_id: &self.command_id,
            speaker: &mut self.speaker,
            brain: &mut self.brain,
            runner: &mut self.runner,
            agent: &self.agent,
        };
        apply_turn_effects(&mut ctx, effects, epoch, clock, &mut |_| {});
    }

    pub fn runner_state(&self) -> State {
        self.runner.state()
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
        on_first_audio: impl FnMut(&str),
    ) -> String {
        let mut ctx = RunContext {
            timing: &mut self.timing,
            command_id: &self.command_id,
            speaker: &mut self.speaker,
            brain: &mut self.brain,
            runner: &mut self.runner,
            agent: &self.agent,
        };
        run_post_hotkey_up(
            &mut ctx,
            PostHotkeyInput { live, final_text, live_match, final_match, epoch, clock, llm_mode, ollama_up },
            on_first_audio,
        )
    }
}
