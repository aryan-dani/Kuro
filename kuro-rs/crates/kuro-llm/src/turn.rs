//! Agent turn runner: FeatureRegistry execution with confirm policy (phase 26A).

use kuro_core::{ActionSource, PermissionTier, needs_confirm};
use kuro_features::{FeatureContext, FeatureError, FeatureRegistry};
use kuro_turn::RouteOutcome;

use crate::agent::AgentConfig;
use crate::agent_llm::AgentLlm;
use crate::audit::{AuditEntry, AuditLog};
use crate::tool_plan::ToolPlan;

const AUDIT_SOURCE: &str = "llm";

/// Outcome when `AgentConfig::enabled` is true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRunResult {
    pub speeches: Vec<String>,
    pub pending_confirm: Option<RouteOutcome>,
    pub audit: AuditLog,
    pub steps_run: u32,
}

/// Run the capped agent loop. Returns `None` when the agent is disabled.
pub fn run_agent_turn(
    config: &AgentConfig,
    llm: &dyn AgentLlm,
    registry: &FeatureRegistry,
    ctx: &FeatureContext<'_>,
    user_text: &str,
    turn_id: u64,
    plan_id: u64,
) -> Option<AgentRunResult> {
    if !config.enabled {
        return None;
    }

    let mut audit = AuditLog::new();
    let mut speeches = Vec::new();
    let mut steps_run = 0;

    for _ in 0..config.max_steps {
        let plans = llm.plan_tools(user_text);
        if plans.is_empty() {
            break;
        }
        steps_run += 1;

        for plan in plans {
            if let Some(pending) = apply_plan(registry, ctx, &plan, turn_id, plan_id, &mut audit, &mut speeches) {
                return Some(AgentRunResult { speeches, pending_confirm: Some(pending), audit, steps_run });
            }
        }
    }

    Some(AgentRunResult { speeches, pending_confirm: None, audit, steps_run })
}

fn apply_plan(
    registry: &FeatureRegistry,
    ctx: &FeatureContext<'_>,
    plan: &ToolPlan,
    turn_id: u64,
    plan_id: u64,
    audit: &mut AuditLog,
    speeches: &mut Vec<String>,
) -> Option<RouteOutcome> {
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

    let tier = PermissionTier::parse(&feature.manifest().tier).unwrap_or(PermissionTier::Irreversible);
    let tier_label = tier.manifest_str().to_string();

    if needs_confirm(tier, ActionSource::Llm) {
        audit.push(AuditEntry {
            turn_id,
            source: AUDIT_SOURCE,
            feature_id: plan.feature_id.clone(),
            tier: tier_label.clone(),
            result: "confirm_pending".into(),
        });
        let summary = if plan.summary.is_empty() { plan.feature_id.clone() } else { plan.summary.clone() };
        return Some(RouteOutcome {
            reply: format!("Confirm: {summary}"),
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
