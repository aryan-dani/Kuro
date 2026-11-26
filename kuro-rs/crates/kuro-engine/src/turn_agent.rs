//! Agent planner before conversational LLM (phase 26 wiring).

use std::sync::Mutex;

use kuro_features::{FeatureContext, FeatureRegistry, SystemClock};
use kuro_llm::{AgentConfig, FakeLlm, PlannerLlm, ToolPlan, execute_confirmed_plan, run_agent_turn};
use kuro_router::RouteResult;
use kuro_turn::RouteOutcome;
use serde_json::json;

pub struct AgentHook {
    pub config: AgentConfig,
    pub registry: FeatureRegistry,
    pub clock: SystemClock,
    pub pending_plan: Mutex<Option<ToolPlan>>,
    pub test_fake: Option<FakeLlm>,
}

impl Default for AgentHook {
    fn default() -> Self {
        Self {
            config: AgentConfig::default(),
            registry: crate::live_controls::live_registry(),
            clock: SystemClock,
            pending_plan: Mutex::new(None),
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

    fn take_pending(&self) -> Option<ToolPlan> {
        self.pending_plan.lock().ok().and_then(|mut slot| slot.take())
    }

    fn set_pending(&self, plan: ToolPlan) {
        if let Ok(mut slot) = self.pending_plan.lock() {
            *slot = Some(plan);
        }
    }
}

#[derive(Debug)]
pub(crate) enum AgentRoute {
    Reply(String, bool),
    Confirm(RouteOutcome),
}

fn is_confirm_yes(text: &str) -> bool {
    let t = kuro_router::normalize(text);
    matches!(t.as_str(), "yes" | "yeah" | "yep" | "sure" | "ok" | "okay" | "do it" | "go ahead" | "send it" | "confirm")
}

pub(crate) fn try_agent_route(
    hook: &AgentHook,
    text: &str,
    routed: &RouteResult,
    llm_mode: &str,
    turn_id: u64,
    plan_id: u64,
) -> Option<AgentRoute> {
    if let Some(plan) = hook.take_pending() {
        if is_confirm_yes(text) {
            let ctx = FeatureContext::new(&hook.clock);
            let spoken = execute_confirmed_plan(&hook.registry, &ctx, &plan);
            return Some(AgentRoute::Reply(spoken, false));
        }
        return Some(AgentRoute::Reply("Cancelled.".into(), false));
    }
    // route_command already ran the phrase. Calling try_handle again created a second Meet.
    if !routed.reply.is_empty() {
        return None;
    }
    // Hybrid: soft Meet follow-ups work even when the full agent loop is off.
    if let Some(follow) = crate::meet_followup_route::try_soft_meet_followup(text, routed.skip_tts) {
        return Some(follow);
    }
    if llm_mode == "off" || !routed.start_ollama {
        return None;
    }
    let ctx = FeatureContext::new(&hook.clock);
    let active_cfg = AgentConfig { enabled: true, max_steps: hook.config.max_steps.max(1) };
    let result = if let Some(fake) = hook.test_fake.as_ref() {
        if std::env::var("KURO_AGENT_FAKE").ok().as_deref() == Some("1") || hook.config.enabled {
            run_agent_turn(&active_cfg, fake, &hook.registry, &ctx, text, turn_id, plan_id)?
        } else {
            return None;
        }
    } else {
        let catalog = hook.registry.catalog_prompt();
        let book = kuro_tools::whatsapp::load_contacts();
        let contacts = kuro_tools::contact_bias::stt_contact_names(&book);
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:e4b".into());
        let planner = PlannerLlm::new(&catalog, &contacts, move |system, user| {
            let messages =
                vec![json!({ "role": "system", "content": system }), json!({ "role": "user", "content": user })];
            kuro_tools::brain::chat(&model, &messages)
        });
        run_agent_turn(&active_cfg, &planner, &hook.registry, &ctx, text, turn_id, plan_id)?
    };
    if let Some(pending) = result.pending_confirm {
        if let Some(plan) = result.pending_plan {
            hook.set_pending(plan);
        }
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

    #[test]
    fn planned_whatsapp_confirms_then_sends_only_on_yes() {
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_c = hits.clone();
        kuro_tools::set_executor(Some(Box::new(move |name, _| {
            if name == "send_whatsapp" {
                hits_c.fetch_add(1, Ordering::SeqCst);
            }
            "Success: Sent that to Siya.".into()
        })));
        let plan = ToolPlan {
            feature_id: "whatsapp".into(),
            slots: json!({ "contact": "Siya", "source": "text", "text": "Are you okay?" }),
            summary: "Message Siya on WhatsApp: Are you okay?".into(),
        };
        let hook = AgentHook::from_config(AgentConfig { enabled: true, max_steps: 2 })
            .with_fake(FakeLlm::scripted_plans(vec![vec![plan]]));
        let routed = RouteResult { reply: String::new(), skip_tts: false, force_fast: false, start_ollama: true };
        let first = try_agent_route(&hook, "check on Siya on WhatsApp", &routed, "fallback", 1, 1);
        assert!(matches!(first, Some(AgentRoute::Confirm(_))), "expected Confirm, got {first:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "must not send before yes");

        let second = try_agent_route(&hook, "yes", &routed, "fallback", 2, 2);
        assert!(matches!(second, Some(AgentRoute::Reply(ref s, _)) if s.contains("Sent")), "got {second:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 1, "yes must send once");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn planned_whatsapp_no_cancels_without_sending() {
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_c = hits.clone();
        kuro_tools::set_executor(Some(Box::new(move |name, _| {
            if name == "send_whatsapp" {
                hits_c.fetch_add(1, Ordering::SeqCst);
            }
            "Success: Sent that to Siya.".into()
        })));
        let plan = ToolPlan {
            feature_id: "whatsapp".into(),
            slots: json!({ "contact": "Siya", "source": "text", "text": "Are you okay?" }),
            summary: "Message Siya on WhatsApp: Are you okay?".into(),
        };
        let hook = AgentHook::from_config(AgentConfig { enabled: true, max_steps: 2 })
            .with_fake(FakeLlm::scripted_plans(vec![vec![plan]]));
        let routed = RouteResult { reply: String::new(), skip_tts: false, force_fast: false, start_ollama: true };
        let _ = try_agent_route(&hook, "check on Siya on WhatsApp", &routed, "fallback", 1, 1);
        let second = try_agent_route(&hook, "no", &routed, "fallback", 2, 2);
        assert!(matches!(second, Some(AgentRoute::Reply(ref s, _)) if s == "Cancelled."), "got {second:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "no must not send");
        kuro_tools::set_executor(None);
    }
}
