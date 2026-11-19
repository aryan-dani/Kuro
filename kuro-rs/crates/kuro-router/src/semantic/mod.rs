//! Tiered semantic intent router: exact hash, optional ONNX/hash cosine, margin guard.

mod decide;
mod embed_cache;
mod exemplars;
mod guards;
mod similarity;
mod tier1;

pub use decide::{MARGIN_INTENT, MARGIN_LLM, TAU_INTENT, decide_from_matrix, decide_from_scores};
pub use embed_cache::{find_onnx, hash_embed, invalidate_exemplar_cache, static_exemplars, unload};
pub use guards::{conversational_help_suffix, looks_like_question};
pub use similarity::{EMBED_DIM, cosine_similarity, cosine_similarity_384, l2_normalize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    Cancel,
    Thanks,
    Help,
    Identity,
    Time,
    Date,
    PauseMusic,
    ResumeMusic,
    StopMusic,
    GoodNight,
    DaddyHome,
    PowerOff,
    PowerOn,
    GetNews,
    MakeMeet,
    CheckInbox,
    SendWhatsapp,
    RunWorkflow,
    ReadScreen,
    ConversationalLlm,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RouteDecision {
    /// Fall through to Ollama / no local phrase.
    Llm,
    /// Exact or semantic tool hit.
    Tool(Intent),
}

/// Public entry: tier-1 exact, conversational guard, optional embedding path, else None (legacy).
pub fn route_semantic(normalized: &str) -> Option<RouteDecision> {
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    if tokens.len() <= 3
        && let Some(intent) = tier1::tier1_map().get(normalized).copied()
    {
        return Some(RouteDecision::Tool(intent));
    }
    if conversational_help_suffix(normalized) {
        return Some(RouteDecision::Llm);
    }
    if looks_like_question(normalized) {
        return Some(RouteDecision::Llm);
    }
    embed_cache::try_embed_route(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_is_one() {
        let mut a = [0.0f32; 8];
        a[0] = 3.0;
        a[1] = 4.0;
        l2_normalize(&mut a);
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn long_help_suffix_forces_llm() {
        let t = "i wanted to understand how to build a large language model from scratch can you help me";
        assert!(conversational_help_suffix(t));
        assert_eq!(decide_from_scores(Some((Intent::Help, 0.95)), Some(0.5), 0.1, t), RouteDecision::Llm);
    }

    #[test]
    fn margin_rejects_ambiguous() {
        assert_eq!(
            decide_from_scores(Some((Intent::PowerOff, 0.80)), Some(0.79), 0.1, "desk power"),
            RouteDecision::Llm
        );
    }

    #[test]
    fn strong_tool_wins() {
        assert_eq!(
            decide_from_scores(Some((Intent::PowerOn, 0.90)), Some(0.50), 0.40, "desk on"),
            RouteDecision::Tool(Intent::PowerOn)
        );
    }

    #[test]
    fn llm_anchor_blocks_help() {
        assert_eq!(decide_from_scores(Some((Intent::Help, 0.85)), Some(0.40), 0.84, "short help"), RouteDecision::Llm);
    }

    #[test]
    fn tier1_exact_desk_on() {
        assert_eq!(route_semantic("desk on"), Some(RouteDecision::Tool(Intent::PowerOn)));
        assert_eq!(route_semantic("help"), Some(RouteDecision::Tool(Intent::Help)));
    }
}
