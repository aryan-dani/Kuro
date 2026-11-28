use kuro_features::{FakeClock, FeatureContext, FeatureRegistry};
use serde_json::json;

use crate::agent::AgentConfig;
use crate::agent_llm::AgentLlm;
use crate::fake::FakeLlm;
use crate::tool_plan::ToolPlan;
use crate::turn::{TurnRequest, run_agent_request, run_agent_turn};

fn with_ctx(run: impl FnOnce(&FeatureContext<'_>)) {
    let clock = FakeClock::at(chrono::Utc::now());
    let ctx = FeatureContext::new(&clock);
    run(&ctx);
}

fn llm_with_plan_batches(batches: Vec<Vec<ToolPlan>>) -> FakeLlm {
    FakeLlm::scripted_plans(batches)
}

#[test]
fn agent_disabled_returns_none() {
    let config = AgentConfig::default();
    let llm = FakeLlm::scripted_plans([Vec::<ToolPlan>::new()]);
    let registry = FeatureRegistry::with_features();
    with_ctx(|ctx| {
        let out = run_agent_turn(&config, &llm, &registry, ctx, "hi", 1, 1);
        assert!(out.is_none());
    });
}

#[test]
fn desk_then_whatsapp_confirms_second_plan() {
    let config = AgentConfig { enabled: true, max_steps: 3 };
    let plans = vec![
        ToolPlan {
            feature_id: "desk".into(),
            slots: json!({"scene": "power_on"}),
            summary: "Turn the desk on.".into(),
        },
        ToolPlan {
            feature_id: "whatsapp".into(),
            slots: json!({"contact": "sam", "source": "clipboard", "text": "hi"}),
            summary: "Send WhatsApp to Sam.".into(),
        },
    ];
    let llm = llm_with_plan_batches(vec![plans]);
    let registry = FeatureRegistry::with_features();
    with_ctx(|ctx| {
        let out = run_agent_turn(&config, &llm, &registry, ctx, "run desk and whatsapp", 9, 100).expect("enabled");
        assert_eq!(out.speeches, vec!["Desk on."]);
        let pending = out.pending_confirm.expect("confirm");
        assert!(pending.needs_confirm);
        assert_eq!(pending.confirm_tier, "external_send");
        assert_eq!(pending.confirm_summary, "Send WhatsApp to Sam.");
        assert_eq!(pending.plan_id, 100);
        assert_eq!(out.audit.entries().len(), 2);
        assert_eq!(out.audit.entries()[0].result, "ok");
        assert_eq!(out.audit.entries()[1].result, "confirm_pending");
    });
}

#[test]
fn unknown_feature_speaks_reject_without_panic() {
    let config = AgentConfig { enabled: true, max_steps: 1 };
    let plans = vec![ToolPlan { feature_id: "not_a_feature".into(), slots: json!(null), summary: String::new() }];
    let llm = llm_with_plan_batches(vec![plans]);
    let registry = FeatureRegistry::with_features();
    with_ctx(|ctx| {
        let out = run_agent_turn(&config, &llm, &registry, ctx, "x", 2, 1).expect("enabled");
        assert!(out.pending_confirm.is_none());
        assert_eq!(out.speeches.len(), 1);
        assert!(out.speeches[0].contains("unknown feature"));
        assert_eq!(out.audit.entries()[0].result, "rejected");
    });
}

#[test]
fn max_steps_caps_llm_plan_rounds() {
    let config = AgentConfig { enabled: true, max_steps: 3 };
    let step_plan = vec![ToolPlan { feature_id: "time".into(), slots: json!(null), summary: String::new() }];
    let llm = llm_with_plan_batches(vec![step_plan.clone(), step_plan.clone(), step_plan.clone(), step_plan]);
    let registry = FeatureRegistry::with_features();
    with_ctx(|ctx| {
        let out = run_agent_turn(&config, &llm, &registry, ctx, "repeat", 3, 1).expect("enabled");
        assert_eq!(out.steps_run, 3);
        assert_eq!(out.speeches.len(), 3);
        assert!(out.pending_confirm.is_none());
    });
}

#[test]
fn fake_llm_parses_json_plans_from_complete() {
    let json_line = r#"[{"feature_id":"thanks","slots":{},"summary":"Say thanks."}]"#;
    let llm = FakeLlm::scripted([json_line]);
    let plans = llm.plan_tools_via_complete("thanks");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].feature_id, "thanks");
}

#[test]
fn agent_llm_default_plan_tools_empty_for_plain_fake() {
    let llm = FakeLlm::scripted(["hello"]);
    assert!(llm.plan_tools("hi").is_empty());
}

#[test]
fn executes_reversible_without_confirm() {
    let config = AgentConfig { enabled: true, max_steps: 1 };
    let plans =
        vec![ToolPlan { feature_id: "desk".into(), slots: json!({"scene": "power_off"}), summary: String::new() }];
    let llm = llm_with_plan_batches(vec![plans]);
    let registry = FeatureRegistry::with_features();
    with_ctx(|ctx| {
        let out = run_agent_turn(&config, &llm, &registry, ctx, "desk off", 4, 7).expect("enabled");
        assert_eq!(out.speeches, vec!["Desk off."]);
        assert!(out.pending_confirm.is_none());
    });
}

fn whatsapp_plan(source: &str, text: &str) -> ToolPlan {
    ToolPlan {
        feature_id: "whatsapp".into(),
        slots: json!({"contact": "Siya", "source": source, "text": text}),
        summary: "Message Siya on WhatsApp.".into(),
    }
}

#[test]
fn saved_contact_named_in_request_sends_without_confirm() {
    let config = AgentConfig { enabled: true, max_steps: 1 };
    let llm = llm_with_plan_batches(vec![vec![whatsapp_plan("text", "How are you?")]]);
    let registry = FeatureRegistry::with_features();
    let direct = vec!["Siya".to_string()];
    let req =
        TurnRequest { user_text: "Hey Kuro. Ask Siya how she is.", turn_id: 5, plan_id: 11, direct_contacts: &direct };
    with_ctx(|ctx| {
        let out = run_agent_request(&config, &llm, &registry, ctx, &req).expect("enabled");
        assert!(out.pending_confirm.is_none(), "saved contact named by the user should not need confirm");
        assert_eq!(out.audit.entries().len(), 1);
        assert_ne!(out.audit.entries()[0].result, "confirm_pending");
    });
}

#[test]
fn saved_contact_not_named_in_request_still_confirms() {
    let config = AgentConfig { enabled: true, max_steps: 1 };
    let llm = llm_with_plan_batches(vec![vec![whatsapp_plan("text", "How are you?")]]);
    let registry = FeatureRegistry::with_features();
    let direct = vec!["Siya".to_string()];
    let req = TurnRequest {
        user_text: "Hey Kuro. Ask my sister how she is.",
        turn_id: 5,
        plan_id: 11,
        direct_contacts: &direct,
    };
    with_ctx(|ctx| {
        let out = run_agent_request(&config, &llm, &registry, ctx, &req).expect("enabled");
        assert!(out.pending_confirm.is_some(), "model guessed a contact the user did not say");
    });
}

#[test]
fn unknown_contact_confirms_and_reads_exact_message() {
    let config = AgentConfig { enabled: true, max_steps: 1 };
    let llm = llm_with_plan_batches(vec![vec![whatsapp_plan("Kuro", "How are you?")]]);
    let registry = FeatureRegistry::with_features();
    let req = TurnRequest { user_text: "Ask Siya how she is.", turn_id: 5, plan_id: 11, direct_contacts: &[] };
    with_ctx(|ctx| {
        let out = run_agent_request(&config, &llm, &registry, ctx, &req).expect("enabled");
        let pending = out.pending_confirm.expect("confirm");
        assert!(pending.confirm_summary.contains("\"How are you?\""), "got {}", pending.confirm_summary);
        assert!(pending.reply.contains("Say yes"), "got {}", pending.reply);
        let plan = out.pending_plan.expect("plan");
        assert_eq!(plan.slots["source"], "text", "invalid source should fall back to text");
    });
}
