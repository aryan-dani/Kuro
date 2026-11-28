//! Agent planner before conversational LLM (phase 26 wiring).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use kuro_features::{FeatureContext, FeatureRegistry, SystemClock};
use kuro_llm::{AgentConfig, FakeLlm, PlannerLlm, ToolPlan, TurnRequest, execute_confirmed_plan, run_agent_request};
use kuro_router::RouteResult;
use kuro_turn::RouteOutcome;
use serde_json::json;

/// How long a pending confirmation waits for yes or no before it lapses.
const PENDING_TTL: Duration = Duration::from_secs(20);
const YES_WORDS: &[&str] = &["yes", "yeah", "yep", "yup", "sure", "ok", "okay", "confirm"];
const YES_PHRASES: &[&str] = &["send it", "send that", "do it", "go ahead"];
const NO_WORDS: &[&str] = &["no", "nope", "nah", "cancel", "stop", "dont", "don't"];

struct Pending {
    plan: ToolPlan,
    at: Instant,
}

pub struct AgentHook {
    pub config: AgentConfig,
    pub registry: FeatureRegistry,
    pub clock: SystemClock,
    pending_plan: Mutex<Option<Pending>>,
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

    fn take_pending(&self) -> Option<Pending> {
        self.pending_plan.lock().ok().and_then(|mut slot| slot.take())
    }

    fn set_pending(&self, plan: ToolPlan) {
        if let Ok(mut slot) = self.pending_plan.lock() {
            *slot = Some(Pending { plan, at: Instant::now() });
        }
    }
}

#[derive(Debug)]
pub(crate) enum AgentRoute {
    Reply(String, bool),
    Confirm(RouteOutcome),
}

#[derive(Debug, PartialEq, Eq)]
enum Answer {
    Yes,
    No,
    Other,
}

/// Classify the answer to a pending confirmation. Long sentences are never a yes.
fn classify(text: &str) -> Answer {
    let t = kuro_router::normalize(text);
    let first = t.split_whitespace().next().unwrap_or("");
    let words = t.split_whitespace().count();
    if (YES_WORDS.contains(&first) && words <= 3) || YES_PHRASES.contains(&t.as_str()) {
        Answer::Yes
    } else if NO_WORDS.contains(&first) && words <= 3 {
        Answer::No
    } else {
        Answer::Other
    }
}

pub(crate) fn try_agent_route(
    hook: &AgentHook,
    text: &str,
    routed: &RouteResult,
    llm_mode: &str,
    turn_id: u64,
    plan_id: u64,
) -> Option<AgentRoute> {
    if let Some(pending) = hook.take_pending()
        && pending.at.elapsed() <= PENDING_TTL
    {
        match classify(text) {
            Answer::Yes => {
                let ctx = FeatureContext::new(&hook.clock);
                let spoken = execute_confirmed_plan(&hook.registry, &ctx, &pending.plan);
                return Some(AgentRoute::Reply(spoken, false));
            }
            Answer::No => return Some(AgentRoute::Reply("Cancelled.".into(), false)),
            // Unrelated speech drops the plan and is handled as a new request.
            Answer::Other => {}
        }
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
            let req = TurnRequest { user_text: text, turn_id, plan_id, direct_contacts: &[] };
            run_agent_request(&active_cfg, fake, &hook.registry, &ctx, &req)?
        } else {
            return None;
        }
    } else {
        let catalog = hook.registry.catalog_prompt();
        let book = kuro_tools::whatsapp::load_contacts();
        let contacts = kuro_tools::contact_bias::stt_contact_names(&book);
        let direct: Vec<String> = book.keys().cloned().collect();
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:e4b".into());
        let planner = PlannerLlm::new(&catalog, &contacts, move |system, user| {
            let messages =
                vec![json!({ "role": "system", "content": system }), json!({ "role": "user", "content": user })];
            kuro_tools::brain::chat(&model, &messages)
        });
        let req = TurnRequest { user_text: text, turn_id, plan_id, direct_contacts: &direct };
        run_agent_request(&active_cfg, &planner, &hook.registry, &ctx, &req)?
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

    fn whatsapp_plan() -> ToolPlan {
        ToolPlan {
            feature_id: "whatsapp".into(),
            slots: json!({ "contact": "Siya", "source": "text", "text": "Are you okay?" }),
            summary: "Message Siya on WhatsApp: Are you okay?".into(),
        }
    }

    fn routed_ollama() -> RouteResult {
        RouteResult { reply: String::new(), skip_tts: false, force_fast: false, start_ollama: true }
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
        let hook = AgentHook::from_config(AgentConfig { enabled: true, max_steps: 2 })
            .with_fake(FakeLlm::scripted_plans(vec![vec![whatsapp_plan()]]));
        let routed = routed_ollama();
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
        let hook = AgentHook::from_config(AgentConfig { enabled: true, max_steps: 2 })
            .with_fake(FakeLlm::scripted_plans(vec![vec![whatsapp_plan()]]));
        let routed = routed_ollama();
        let _ = try_agent_route(&hook, "check on Siya on WhatsApp", &routed, "fallback", 1, 1);
        let second = try_agent_route(&hook, "no", &routed, "fallback", 2, 2);
        assert!(matches!(second, Some(AgentRoute::Reply(ref s, _)) if s == "Cancelled."), "got {second:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "no must not send");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn expired_confirmation_does_not_send_on_yes() {
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_c = hits.clone();
        kuro_tools::set_executor(Some(Box::new(move |name, _| {
            if name == "send_whatsapp" {
                hits_c.fetch_add(1, Ordering::SeqCst);
            }
            "Success: Sent that to Siya.".into()
        })));
        let Some(old) = Instant::now().checked_sub(PENDING_TTL + Duration::from_secs(5)) else {
            kuro_tools::set_executor(None);
            return;
        };
        let hook = AgentHook::from_config(AgentConfig { enabled: true, max_steps: 2 })
            .with_fake(FakeLlm::scripted_plans(Vec::<Vec<ToolPlan>>::new()));
        *hook.pending_plan.lock().unwrap() = Some(Pending { plan: whatsapp_plan(), at: old });
        let second = try_agent_route(&hook, "yes", &routed_ollama(), "fallback", 2, 2);
        assert!(!matches!(second, Some(AgentRoute::Reply(ref s, _)) if s.contains("Sent")), "got {second:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "stale yes must not send");
        kuro_tools::set_executor(None);
    }

    #[test]
    fn unrelated_speech_drops_pending_plan_without_sending() {
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_c = hits.clone();
        kuro_tools::set_executor(Some(Box::new(move |name, _| {
            if name == "send_whatsapp" {
                hits_c.fetch_add(1, Ordering::SeqCst);
            }
            "Success: Sent that to Siya.".into()
        })));
        let hook = AgentHook::from_config(AgentConfig { enabled: true, max_steps: 2 })
            .with_fake(FakeLlm::scripted_plans(Vec::<Vec<ToolPlan>>::new()));
        *hook.pending_plan.lock().unwrap() = Some(Pending { plan: whatsapp_plan(), at: Instant::now() });
        let other = try_agent_route(
            &hook,
            "How you are. Kuchy priority. Don't worry about it.",
            &routed_ollama(),
            "fallback",
            2,
            2,
        );
        assert!(other.is_none(), "unrelated speech is a new request, got {other:?}");
        assert!(hook.take_pending().is_none(), "unrelated speech must clear the plan");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        kuro_tools::set_executor(None);
    }

    #[test]
    fn yes_with_please_counts_as_yes() {
        assert_eq!(classify("Yes, please."), Answer::Yes);
        assert_eq!(classify("send it"), Answer::Yes);
        assert_eq!(classify("No thanks"), Answer::No);
        assert_eq!(classify("yes but change the message to something else entirely"), Answer::Other);
    }
}
