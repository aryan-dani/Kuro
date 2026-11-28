//! Agent turn runner: FeatureRegistry execution with confirm policy (phase 26A).

use kuro_core::{ActionSource, PermissionTier, needs_confirm};
use kuro_features::{FeatureContext, FeatureError, FeatureRegistry};
use kuro_turn::RouteOutcome;
use serde_json::json;

use crate::agent::AgentConfig;
use crate::agent_llm::AgentLlm;
use crate::audit::{AuditEntry, AuditLog};
use crate::tool_plan::ToolPlan;

const AUDIT_SOURCE: &str = "llm";
const WHATSAPP_SOURCES: &[&str] = &["text", "clipboard", "browser_url"];

/// One agent request: what the user said and the saved contacts that may be messaged directly.
pub struct TurnRequest<'a> {
    pub user_text: &'a str,
    pub turn_id: u64,
    pub plan_id: u64,
    /// Exact saved contact names. A WhatsApp text to one of these runs without confirmation
    /// only when the user named that contact in their own words.
    pub direct_contacts: &'a [String],
}

/// Outcome when `AgentConfig::enabled` is true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRunResult {
    pub speeches: Vec<String>,
    pub pending_confirm: Option<RouteOutcome>,
    pub pending_plan: Option<ToolPlan>,
    pub audit: AuditLog,
    pub steps_run: u32,
}

/// Run the capped agent loop with no direct-send contacts. Returns `None` when the agent is disabled.
pub fn run_agent_turn(
    config: &AgentConfig,
    llm: &dyn AgentLlm,
    registry: &FeatureRegistry,
    ctx: &FeatureContext<'_>,
    user_text: &str,
    turn_id: u64,
    plan_id: u64,
) -> Option<AgentRunResult> {
    let req = TurnRequest { user_text, turn_id, plan_id, direct_contacts: &[] };
    run_agent_request(config, llm, registry, ctx, &req)
}

/// Run the capped agent loop for one request. Returns `None` when the agent is disabled.
pub fn run_agent_request(
    config: &AgentConfig,
    llm: &dyn AgentLlm,
    registry: &FeatureRegistry,
    ctx: &FeatureContext<'_>,
    req: &TurnRequest<'_>,
) -> Option<AgentRunResult> {
    if !config.enabled {
        return None;
    }

    let mut audit = AuditLog::new();
    let mut speeches = Vec::new();
    let mut steps_run = 0;

    for _ in 0..config.max_steps {
        let plans = llm.plan_tools(req.user_text);
        if plans.is_empty() {
            break;
        }
        steps_run += 1;

        for mut plan in plans {
            normalize_slots(&mut plan);
            if let Some(pending) = apply_plan(registry, ctx, &plan, req, &mut audit, &mut speeches) {
                return Some(AgentRunResult {
                    speeches,
                    pending_confirm: Some(pending),
                    pending_plan: Some(plan),
                    audit,
                    steps_run,
                });
            }
        }
    }

    Some(AgentRunResult { speeches, pending_confirm: None, pending_plan: None, audit, steps_run })
}

/// Execute a previously confirmed tool plan directly through the feature registry.
pub fn execute_confirmed_plan(registry: &FeatureRegistry, ctx: &FeatureContext<'_>, plan: &ToolPlan) -> String {
    match registry.execute_spoken(&plan.feature_id, ctx, &plan.slots) {
        Ok(spoken) => spoken,
        Err(err @ FeatureError::InvalidInput { .. }) => err.detail().to_string(),
        Err(_) => "That didn't work.".into(),
    }
}

fn slot_text(plan: &ToolPlan, key: &str) -> String {
    plan.slots.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
}

/// Small models sometimes put a name in `source`. Unknown sources fall back to plain text.
fn normalize_slots(plan: &mut ToolPlan) {
    if plan.feature_id != "whatsapp" {
        return;
    }
    let source = slot_text(plan, "source").to_ascii_lowercase();
    if WHATSAPP_SOURCES.contains(&source.as_str()) {
        return;
    }
    if let Some(obj) = plan.slots.as_object_mut() {
        obj.insert("source".into(), json!("text"));
    }
}

/// True when `name` appears in `text` as whole words, ignoring case and punctuation.
fn mentions_name(text: &str, name: &str) -> bool {
    let norm = |s: &str| {
        let words: Vec<String> = s
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        format!(" {} ", words.join(" "))
    };
    norm(text).contains(&norm(name))
}

/// A WhatsApp text to a saved contact that the user named in this request may send without confirmation.
fn is_direct_send(plan: &ToolPlan, req: &TurnRequest<'_>) -> bool {
    if plan.feature_id != "whatsapp" || slot_text(plan, "source") != "text" {
        return false;
    }
    let contact = slot_text(plan, "contact");
    if contact.is_empty() || slot_text(plan, "text").is_empty() {
        return false;
    }
    let known = req.direct_contacts.iter().any(|name| name.trim().eq_ignore_ascii_case(&contact));
    known && mentions_name(req.user_text, &contact)
}

/// Spoken confirmation. WhatsApp text sends read the exact message back to the user.
fn confirm_summary(plan: &ToolPlan) -> String {
    if plan.feature_id == "whatsapp" && slot_text(plan, "source") == "text" {
        let contact = slot_text(plan, "contact");
        let text = slot_text(plan, "text");
        if !contact.is_empty() && !text.is_empty() {
            return format!("send \"{text}\" to {contact} on WhatsApp");
        }
    }
    if plan.summary.is_empty() { plan.feature_id.clone() } else { plan.summary.clone() }
}

fn effective_tier(feature_tier: &str, plan: &ToolPlan) -> PermissionTier {
    let base = PermissionTier::parse(feature_tier).unwrap_or(PermissionTier::Irreversible);
    if plan.feature_id == "meet" {
        let has_contact = plan.slots.get("contact").and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty());
        let is_join = plan.slots.get("action").and_then(|v| v.as_str()) == Some("join");
        if has_contact && !is_join {
            return PermissionTier::ExternalSend;
        }
    }
    base
}

fn apply_plan(
    registry: &FeatureRegistry,
    ctx: &FeatureContext<'_>,
    plan: &ToolPlan,
    req: &TurnRequest<'_>,
    audit: &mut AuditLog,
    speeches: &mut Vec<String>,
) -> Option<RouteOutcome> {
    let (turn_id, plan_id) = (req.turn_id, req.plan_id);
    let Some(feature) = registry.get(&plan.feature_id) else {
        let speech = format!("I can't run that: unknown feature {}.", plan.feature_id);
        speeches.push(speech);
        audit.push(AuditEntry {
            turn_id,
            source: AUDIT_SOURCE,
            feature_id: plan.feature_id.clone(),
            tier: "unknown".into(),
            result: "rejected".into(),
        });
        return None;
    };

    let tier = effective_tier(&feature.manifest().tier, plan);
    let tier_label = tier.manifest_str().to_string();

    if needs_confirm(tier, ActionSource::Llm) && !is_direct_send(plan, req) {
        audit.push(AuditEntry {
            turn_id,
            source: AUDIT_SOURCE,
            feature_id: plan.feature_id.clone(),
            tier: tier_label.clone(),
            result: "confirm_pending".into(),
        });
        let summary = confirm_summary(plan);
        return Some(RouteOutcome {
            reply: format!("Confirm: {summary}. Say yes to send or no to cancel."),
            needs_execute: true,
            skip_tts: false,
            allow_follow_up: false,
            needs_confirm: true,
            confirm_summary: summary,
            confirm_tier: tier_label,
            plan_id,
        });
    }

    match registry.execute_spoken(&plan.feature_id, ctx, &plan.slots) {
        Ok(spoken) => {
            speeches.push(spoken);
            audit.push(AuditEntry {
                turn_id,
                source: AUDIT_SOURCE,
                feature_id: plan.feature_id.clone(),
                tier: tier_label,
                result: "ok".into(),
            });
        }
        Err(err @ FeatureError::InvalidInput { .. }) => {
            speeches.push(err.detail().to_string());
            audit.push(AuditEntry {
                turn_id,
                source: AUDIT_SOURCE,
                feature_id: plan.feature_id.clone(),
                tier: tier_label,
                result: "rejected".into(),
            });
        }
        Err(_) => {
            speeches.push("That didn't work.".into());
            audit.push(AuditEntry {
                turn_id,
                source: AUDIT_SOURCE,
                feature_id: plan.feature_id.clone(),
                tier: tier_label,
                result: "error".into(),
            });
        }
    }
    None
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
