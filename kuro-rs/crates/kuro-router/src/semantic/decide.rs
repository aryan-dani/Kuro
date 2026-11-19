use std::collections::HashMap;

use super::guards::{conversational_help_suffix, polarity_ok};
use super::similarity::cosine_similarity;
use super::{Intent, RouteDecision};

pub const TAU_INTENT: f32 = 0.72;
pub const MARGIN_INTENT: f32 = 0.03;
pub const MARGIN_LLM: f32 = 0.04;

/// Score injected query against injected scores (for tests without ONNX).
pub fn decide_from_scores(
    best_tool: Option<(Intent, f32)>,
    second_tool: Option<f32>,
    best_llm: f32,
    normalized: &str,
) -> RouteDecision {
    if conversational_help_suffix(normalized) {
        return RouteDecision::Llm;
    }
    let Some((intent, score)) = best_tool else {
        return RouteDecision::Llm;
    };
    if score < TAU_INTENT {
        return RouteDecision::Llm;
    }
    let second = second_tool.unwrap_or(0.0);
    if score - second < MARGIN_INTENT {
        return RouteDecision::Llm;
    }
    if best_llm >= score - MARGIN_LLM {
        return RouteDecision::Llm;
    }
    if !polarity_ok(intent, normalized) {
        return RouteDecision::Llm;
    }
    if intent == Intent::ConversationalLlm {
        return RouteDecision::Llm;
    }
    RouteDecision::Tool(intent)
}

/// Rank a query vector against an exemplar matrix and apply margin guards.
pub fn decide_from_matrix(query: &[f32], matrix: &[(Intent, Vec<f32>)], normalized: &str) -> RouteDecision {
    let mut best_llm = 0.0f32;
    let mut per_intent: HashMap<Intent, f32> = HashMap::new();
    for (intent, row) in matrix {
        let s = cosine_similarity(query, row);
        if *intent == Intent::ConversationalLlm {
            best_llm = best_llm.max(s);
            continue;
        }
        let entry = per_intent.entry(*intent).or_insert(0.0);
        *entry = (*entry).max(s);
    }
    let mut ranked: Vec<(Intent, f32)> = per_intent.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let best_tool = ranked.first().copied();
    let second = ranked.get(1).map(|p| p.1);
    decide_from_scores(best_tool, second, best_llm, normalized)
}
