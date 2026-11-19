//! Optional agent loop before conversational LLM (phase 26 wiring).

use kuro_features::{FeatureContext, FeatureRegistry, SystemClock};
use kuro_llm::{AgentConfig, AgentLlm, FakeLlm, NullLlm, run_agent_turn};
use kuro_router::RouteResult;
use kuro_router::try_handle;
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
    if try_handle(text).is_some() || llm_mode == "off" || !routed.reply.is_empty() || !routed.start_ollama {
        return None;
    }
    let ctx = FeatureContext::new(&hook.clock);
    let result = run_agent_turn(&hook.config, hook.llm(), &hook.registry, &ctx, text, turn_id, plan_id)?;
    if let Some(pending) = result.pending_confirm {
        return Some(AgentRoute::Confirm(pending));
    }
    if !result.speeches.is_empty() {
        let reply = result.speeches.join(" ");
        return Some(AgentRoute::Reply(reply, routed.skip_tts));
    }
    None
}
