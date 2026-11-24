//! Optional agent loop before conversational LLM (phase 26 wiring).

use kuro_features::{FeatureContext, FeatureRegistry, SystemClock};
use kuro_llm::{AgentConfig, AgentLlm, FakeLlm, NullLlm, run_agent_turn};
use kuro_router::RouteResult;
use kuro_turn::RouteOutcome;

pub struct AgentHook {
    pub config: AgentConfig,
    pub registry: FeatureRegistry,
    pub clock: SystemClock,
    pub null_llm: NullLlm,
    pub test_fake: Option<FakeLlm>,
}

impl Default for AgentHook {
    fn default() -> Self {
        Self {
            config: AgentConfig::default(),
            registry: FeatureRegistry::with_features(),
            clock: SystemClock,
            null_llm: NullLlm,
            test_fake: None,
        }
    }
}

impl AgentHook {
    pub fn from_config(config: AgentConfig) -> Self {
        Self { config, ..Self::default() }
    }

    #[cfg(test)]
    pub fn with_fake(mut self, fake: FakeLlm) -> Self {
        self.test_fake = Some(fake);
        self
    }

    fn llm(&self) -> &dyn AgentLlm {
        if std::env::var("KURO_AGENT_FAKE").ok().as_deref() == Some("1")
            && let Some(fake) = self.test_fake.as_ref()
        {
            return fake;
        }
        &self.null_llm
    }
}

#[derive(Debug)]
pub(crate) enum AgentRoute {
    Reply(String, bool),
    Confirm(RouteOutcome),
}

pub(crate) fn try_agent_route(
    hook: &AgentHook,
    text: &str,
    routed: &RouteResult,
    llm_mode: &str,
    turn_id: u64,
    plan_id: u64,
) -> Option<AgentRoute> {
    // route_command already ran the phrase. Calling try_handle again created a second Meet.
    if !routed.reply.is_empty() {
        return None;
    }
    // Hybrid: soft Meet follow-ups work even when the full agent loop is off.
    if let Some(follow) = crate::meet_followup_route::try_soft_meet_followup(text, routed.skip_tts) {
        return Some(follow);
    }
    if llm_mode == "off" || !routed.reply.is_empty() || !routed.start_ollama {
        return None;
    }
    let ctx = FeatureContext::new(&hook.clock);
    let result = run_agent_turn(&hook.config, hook.llm(), &hook.registry, &ctx, text, turn_id, plan_id)?;
    if let Some(pending) = result.pending_confirm {
        return Some(AgentRoute::Confirm(pending));
    }
    if !result.speeches.is_empty() {
        let reply = kuro_tools::honesty::scrub_chat_reply(text, &result.speeches.join(" "));
        return Some(AgentRoute::Reply(reply, routed.skip_tts));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static MEET_HITS: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn agent_check_does_not_create_a_second_meet() {
        MEET_HITS.store(0, Ordering::SeqCst);
        kuro_tools::set_executor(Some(Box::new(|name, _| {
            if name == "make_meet" {
                MEET_HITS.fetch_add(1, Ordering::SeqCst);
            }
            "Success: The Meet is open.".into()
        })));
        let text = "make a google meet";
        let routed = kuro_router::route_command(text, true, None, "fallback", false);
        assert_eq!(MEET_HITS.load(Ordering::SeqCst), 1, "phrase route should create one meet");
        assert!(try_agent_route(&AgentHook::default(), text, &routed, "fallback", 1, 1).is_none());
        assert_eq!(MEET_HITS.load(Ordering::SeqCst), 1, "agent check must not create another meet");
        kuro_tools::set_executor(None);
    }
}
