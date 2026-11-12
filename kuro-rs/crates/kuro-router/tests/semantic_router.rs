use kuro_router::semantic::{
    Intent, RouteDecision, conversational_help_suffix, cosine_similarity, decide_from_matrix, decide_from_scores,
    hash_embed, l2_normalize, route_semantic,
};

#[test]
fn margin_math_with_injected_vectors() {
    let mut help = hash_embed("list your capabilities").to_vec();
    let mut llm = hash_embed("can you help me understand transformers").to_vec();
    let mut power = hash_embed("turn on the desk").to_vec();
    l2_normalize(&mut help);
    l2_normalize(&mut llm);
    l2_normalize(&mut power);

    let matrix =
        vec![(Intent::Help, help.clone()), (Intent::ConversationalLlm, llm.clone()), (Intent::PowerOn, power.clone())];

    // Exact help exemplar should prefer Help when query is close and LLM is far.
    let q = help.clone();
    let decision = decide_from_matrix(&q, &matrix, "list your capabilities");
    assert_eq!(decision, RouteDecision::Tool(Intent::Help));

    // Ambiguous: force close scores → LLM.
    assert_eq!(decide_from_scores(Some((Intent::Help, 0.80)), Some(0.79), 0.1, "capabilities"), RouteDecision::Llm);
}

#[test]
fn adversarial_long_help_is_llm() {
    let lines = [
        "i wanted to understand how to build a large language model from scratch can you help me",
        "im stuck on a rust borrow checker error can you help me",
        "can you help me write an email to my professor about deadlines",
        "email my professor about deadlines and can you help me",
        "why does my monitor turn off when i play games please help",
        "borrow checker can you help me",
    ];
    for line in lines {
        let t = kuro_router::normalize(line);
        assert!(
            conversational_help_suffix(&t)
                || matches!(route_semantic(&t), Some(RouteDecision::Llm))
                || kuro_router::try_handle(line).is_none(),
            "expected LLM for: {line}"
        );
        // Must not return the Help phrase reply.
        if let Some(result) = kuro_router::try_handle(line) {
            assert!(!result.reply.to_ascii_lowercase().contains("desk, music, news"), "Help stolen for: {line}");
        }
    }
}

#[test]
fn parity_short_commands_still_route() {
    for (text, intent) in [
        ("cancel", Intent::Cancel),
        ("help", Intent::Help),
        ("desk on", Intent::PowerOn),
        ("desk off", Intent::PowerOff),
        ("stop music", Intent::StopMusic),
        ("good night", Intent::GoodNight),
    ] {
        assert_eq!(route_semantic(text), Some(RouteDecision::Tool(intent)), "tier1 miss: {text}");
    }
}

#[test]
fn cosine_self_similarity() {
    let mut a = [0.0f32; 16];
    a[0] = 1.0;
    a[3] = 2.0;
    l2_normalize(&mut a);
    assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
}

#[test]
fn onnx_cases_optional() {
    if kuro_router::semantic::find_onnx().is_none() {
        eprintln!("router-embed ONNX absent; skipping live embed cases");
        return;
    }
    // When the model directory exists, embedding path is enabled.
    let decision = route_semantic("create a google meet link please");
    assert!(
        matches!(decision, Some(RouteDecision::Tool(Intent::MakeMeet)) | Some(RouteDecision::Llm) | None),
        "unexpected: {decision:?}"
    );
}
