//! Tiered semantic intent router: exact hash, optional ONNX/hash cosine, margin guard.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};

pub const EMBED_DIM: usize = 384;
pub const TAU_INTENT: f32 = 0.72;
pub const MARGIN_INTENT: f32 = 0.03;
pub const MARGIN_LLM: f32 = 0.04;

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

#[derive(Clone, Debug)]
struct Exemplar {
    intent: Intent,
    text: &'static str,
}

static EXEMPLARS: &[Exemplar] = &[
    Exemplar { intent: Intent::Cancel, text: "cancel" },
    Exemplar { intent: Intent::Cancel, text: "cancel that" },
    Exemplar { intent: Intent::Cancel, text: "abort current command" },
    Exemplar { intent: Intent::Cancel, text: "never mind forget it" },
    Exemplar { intent: Intent::Cancel, text: "disregard what I just said" },
    Exemplar { intent: Intent::Cancel, text: "stop" },
    Exemplar { intent: Intent::Cancel, text: "nevermind" },
    Exemplar { intent: Intent::Thanks, text: "thank you kuro" },
    Exemplar { intent: Intent::Thanks, text: "appreciate it" },
    Exemplar { intent: Intent::Thanks, text: "thanks a lot" },
    Exemplar { intent: Intent::Thanks, text: "cheers" },
    Exemplar { intent: Intent::Thanks, text: "thanks" },
    Exemplar { intent: Intent::Help, text: "what commands can I say" },
    Exemplar { intent: Intent::Help, text: "list your capabilities" },
    Exemplar { intent: Intent::Help, text: "what tools do you have" },
    Exemplar { intent: Intent::Help, text: "show me the voice command list" },
    Exemplar { intent: Intent::Help, text: "help" },
    Exemplar { intent: Intent::Help, text: "what can you do" },
    Exemplar { intent: Intent::Help, text: "list commands" },
    Exemplar { intent: Intent::Identity, text: "who are you" },
    Exemplar { intent: Intent::Identity, text: "introduce yourself" },
    Exemplar { intent: Intent::Identity, text: "what system are you" },
    Exemplar { intent: Intent::Identity, text: "whats your name" },
    Exemplar { intent: Intent::Time, text: "what time is it right now" },
    Exemplar { intent: Intent::Time, text: "tell me the current time" },
    Exemplar { intent: Intent::Time, text: "check the clock" },
    Exemplar { intent: Intent::Date, text: "what is todays date" },
    Exemplar { intent: Intent::Date, text: "what day of the week is it" },
    Exemplar { intent: Intent::Date, text: "todays date" },
    Exemplar { intent: Intent::PauseMusic, text: "pause the track" },
    Exemplar { intent: Intent::PauseMusic, text: "hold the music" },
    Exemplar { intent: Intent::PauseMusic, text: "pause the song" },
    Exemplar { intent: Intent::ResumeMusic, text: "resume playback" },
    Exemplar { intent: Intent::ResumeMusic, text: "keep playing the song" },
    Exemplar { intent: Intent::ResumeMusic, text: "resume the song" },
    Exemplar { intent: Intent::StopMusic, text: "kill the music" },
    Exemplar { intent: Intent::StopMusic, text: "stop playing audio" },
    Exemplar { intent: Intent::StopMusic, text: "stop the music" },
    Exemplar { intent: Intent::GoodNight, text: "good night kuro" },
    Exemplar { intent: Intent::GoodNight, text: "I am going to sleep shut everything down" },
    Exemplar { intent: Intent::GoodNight, text: "bedtime turn off the desk and music" },
    Exemplar { intent: Intent::DaddyHome, text: "wake up daddys home" },
    Exemplar { intent: Intent::DaddyHome, text: "Im back home boot up the desk" },
    Exemplar { intent: Intent::DaddyHome, text: "daddy is home turn it all on" },
    Exemplar { intent: Intent::PowerOff, text: "turn off the desk" },
    Exemplar { intent: Intent::PowerOff, text: "kill the workspace lights and monitor" },
    Exemplar { intent: Intent::PowerOff, text: "switch off the extension board" },
    Exemplar { intent: Intent::PowerOff, text: "shut down the smart plug" },
    Exemplar { intent: Intent::PowerOff, text: "desk off" },
    Exemplar { intent: Intent::PowerOn, text: "turn on the desk" },
    Exemplar { intent: Intent::PowerOn, text: "power up the monitor and lights" },
    Exemplar { intent: Intent::PowerOn, text: "switch on the extension board" },
    Exemplar { intent: Intent::PowerOn, text: "boot up my workspace" },
    Exemplar { intent: Intent::PowerOn, text: "desk on" },
    Exemplar { intent: Intent::GetNews, text: "what is in the news today" },
    Exemplar { intent: Intent::GetNews, text: "give me the latest tech headlines" },
    Exemplar { intent: Intent::GetNews, text: "catch me up on world news" },
    Exemplar { intent: Intent::GetNews, text: "any hacker news updates" },
    Exemplar { intent: Intent::GetNews, text: "read me the top stories" },
    Exemplar { intent: Intent::MakeMeet, text: "create a google meet link" },
    Exemplar { intent: Intent::MakeMeet, text: "spin up a new meeting room" },
    Exemplar { intent: Intent::MakeMeet, text: "start a video call link" },
    Exemplar { intent: Intent::MakeMeet, text: "send the google meet link to mom" },
    Exemplar { intent: Intent::CheckInbox, text: "check my inbox for urgent emails" },
    Exemplar { intent: Intent::CheckInbox, text: "do I have any upcoming deadlines in my mail" },
    Exemplar { intent: Intent::CheckInbox, text: "summarize my unread emails" },
    Exemplar { intent: Intent::SendWhatsapp, text: "send a whatsapp message to" },
    Exemplar { intent: Intent::SendWhatsapp, text: "text my clipboard to on whatsapp" },
    Exemplar { intent: Intent::SendWhatsapp, text: "share this browser link on whatsapp with" },
    Exemplar { intent: Intent::ReadScreen, text: "read my screen" },
    Exemplar { intent: Intent::ReadScreen, text: "look at what is on my monitor and explain it" },
    Exemplar { intent: Intent::ReadScreen, text: "what does this error on my display say" },
    Exemplar { intent: Intent::ConversationalLlm, text: "I have a question about code can you help me" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Can you help me understand how this works" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Explain the difference between rust and go" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Help me write a function or debug an issue" },
    Exemplar { intent: Intent::ConversationalLlm, text: "What do you think about large language models" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Tell me a story or fact about space" },
    Exemplar {
        intent: Intent::ConversationalLlm,
        text: "I wanted to understand how to build a large language model from scratch can you help me",
    },
    Exemplar { intent: Intent::ConversationalLlm, text: "Im stuck on a rust borrow checker error can you help me" },
    Exemplar {
        intent: Intent::ConversationalLlm,
        text: "Can you help me write an email to my professor about deadlines",
    },
    Exemplar { intent: Intent::ConversationalLlm, text: "Why does my monitor turn off when I play games" },
    Exemplar { intent: Intent::ConversationalLlm, text: "How do neural networks learn from data" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Whats the best way to learn systems programming" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Please help me reason through this math problem" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Yo kuro whats up good morning" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Can you explain transformers architecture to me" },
    Exemplar { intent: Intent::ConversationalLlm, text: "I need advice on debugging a segfault help me out" },
    Exemplar { intent: Intent::ConversationalLlm, text: "What should I cook for dinner tonight" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Compare postgres and sqlite for a laptop app" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Walk me through implementing backprop please help" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Is it raining outside where I am" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Help me understand borrow checker lifetimes" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Can you draft a cover letter for me" },
    Exemplar { intent: Intent::ConversationalLlm, text: "What is the capital of france" },
    Exemplar { intent: Intent::ConversationalLlm, text: "Teach me about attention mechanisms" },
    Exemplar { intent: Intent::ConversationalLlm, text: "I have a homework question can you help" },
];

fn tier1_map() -> &'static HashMap<&'static str, Intent> {
    static MAP: OnceLock<HashMap<&'static str, Intent>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for (k, v) in [
            ("cancel", Intent::Cancel),
            ("nevermind", Intent::Cancel),
            ("never mind", Intent::Cancel),
            ("forget it", Intent::Cancel),
            ("stop", Intent::Cancel),
            ("abort", Intent::Cancel),
            ("thanks", Intent::Thanks),
            ("thank you", Intent::Thanks),
            ("cheers", Intent::Thanks),
            ("help", Intent::Help),
            ("commands", Intent::Help),
            ("who are you", Intent::Identity),
            ("what are you", Intent::Identity),
            ("time", Intent::Time),
            ("the time", Intent::Time),
            ("date", Intent::Date),
            ("the date", Intent::Date),
            ("pause", Intent::PauseMusic),
            ("pause song", Intent::PauseMusic),
            ("pause music", Intent::PauseMusic),
            ("resume", Intent::ResumeMusic),
            ("resume song", Intent::ResumeMusic),
            ("stop music", Intent::StopMusic),
            ("stop song", Intent::StopMusic),
            ("good night", Intent::GoodNight),
            ("goodnight", Intent::GoodNight),
            ("desk on", Intent::PowerOn),
            ("desk off", Intent::PowerOff),
            ("lights on", Intent::PowerOn),
            ("lights off", Intent::PowerOff),
            ("power on", Intent::PowerOn),
            ("power off", Intent::PowerOff),
        ] {
            m.insert(k, v);
        }
        m
    })
}

/// Dot product for equal-length f32 slices; dispatches to AVX2+FMA when available.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    if a.len() == EMBED_DIM {
        let mut aa = [0.0f32; EMBED_DIM];
        let mut bb = [0.0f32; EMBED_DIM];
        aa.copy_from_slice(a);
        bb.copy_from_slice(b);
        return cosine_similarity_384(&aa, &bb);
    }
    dot_scalar(a, b)
}

pub fn cosine_similarity_384(a: &[f32; EMBED_DIM], b: &[f32; EMBED_DIM]) -> f32 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            // Safety: feature bits checked above.
            return unsafe { dot_384_avx2(a, b) };
        }
    }
    dot_scalar(a, b)
}

fn dot_scalar(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        sum += x * y;
    }
    sum
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn dot_384_avx2(a: &[f32; EMBED_DIM], b: &[f32; EMBED_DIM]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    // Safety: caller verified AVX2+FMA; pointers are 384-aligned enough for loadu.
    unsafe {
        let mut acc0 = _mm256_setzero_ps();
        let mut acc1 = _mm256_setzero_ps();
        let mut acc2 = _mm256_setzero_ps();
        let mut acc3 = _mm256_setzero_ps();
        let mut i = 0usize;
        while i + 32 <= EMBED_DIM {
            let a0 = _mm256_loadu_ps(a.as_ptr().add(i));
            let b0 = _mm256_loadu_ps(b.as_ptr().add(i));
            acc0 = _mm256_fmadd_ps(a0, b0, acc0);
            let a1 = _mm256_loadu_ps(a.as_ptr().add(i + 8));
            let b1 = _mm256_loadu_ps(b.as_ptr().add(i + 8));
            acc1 = _mm256_fmadd_ps(a1, b1, acc1);
            let a2 = _mm256_loadu_ps(a.as_ptr().add(i + 16));
            let b2 = _mm256_loadu_ps(b.as_ptr().add(i + 16));
            acc2 = _mm256_fmadd_ps(a2, b2, acc2);
            let a3 = _mm256_loadu_ps(a.as_ptr().add(i + 24));
            let b3 = _mm256_loadu_ps(b.as_ptr().add(i + 24));
            acc3 = _mm256_fmadd_ps(a3, b3, acc3);
            i += 32;
        }
        let mut acc = _mm256_add_ps(_mm256_add_ps(acc0, acc1), _mm256_add_ps(acc2, acc3));
        while i + 8 <= EMBED_DIM {
            let av = _mm256_loadu_ps(a.as_ptr().add(i));
            let bv = _mm256_loadu_ps(b.as_ptr().add(i));
            acc = _mm256_fmadd_ps(av, bv, acc);
            i += 8;
        }
        let hi = _mm256_extractf128_ps(acc, 1);
        let lo = _mm256_castps256_ps128(acc);
        let sum128 = _mm_add_ps(lo, hi);
        let shuf = _mm_movehdup_ps(sum128);
        let sums = _mm_add_ps(sum128, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let sums2 = _mm_add_ss(sums, shuf2);
        let mut out = 0.0f32;
        _mm_store_ss(&mut out, sums2);
        while i < EMBED_DIM {
            out += a[i] * b[i];
            i += 1;
        }
        out
    }
}

pub fn l2_normalize(v: &mut [f32]) {
    let mut norm = 0.0f32;
    for x in v.iter() {
        norm += x * x;
    }
    let norm = norm.sqrt() + 1e-12;
    for x in v.iter_mut() {
        *x /= norm;
    }
}

/// Polite help suffix on a longer utterance → LLM, never Help.
pub fn conversational_help_suffix(normalized: &str) -> bool {
    let words = normalized.split_whitespace().count();
    if words < 8 {
        return false;
    }
    ["can you help me", "help me out", "please help", "can you help", "help me", "please help me"]
        .iter()
        .any(|s| normalized.ends_with(s) || normalized.contains(&format!("{s} ")))
}

/// Question-shaped utterances that must not steal desk/power tools.
pub fn looks_like_question(normalized: &str) -> bool {
    let first = normalized.split_whitespace().next().unwrap_or("");
    if matches!(first, "why" | "how") && normalized.split_whitespace().count() >= 5 {
        return true;
    }
    // Conversational help / explanation requests that embed tool keywords.
    if conversational_help_suffix(normalized) {
        return true;
    }
    if normalized.contains("when i ") || normalized.contains("when i play") {
        return true;
    }
    // Short adversarial help embeds ("borrow checker can you help me").
    let words = normalized.split_whitespace().count();
    (5..=12).contains(&words)
        && (normalized.contains("can you help")
            || normalized.ends_with("help me")
            || normalized.ends_with("help me out"))
}

fn polarity_ok(intent: Intent, normalized: &str) -> bool {
    let on = ["on", "up", "wake", "start", "enable", "boot"].iter().any(|w| crate::has_word(normalized, w));
    let off = ["off", "down", "kill", "shut", "disable", "sleep"].iter().any(|w| crate::has_word(normalized, w));
    match intent {
        Intent::PowerOn | Intent::DaddyHome => on || !off,
        Intent::PowerOff | Intent::GoodNight => off || !on,
        _ => true,
    }
}

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

/// Public entry: tier-1 exact, conversational guard, optional embedding path, else None (legacy).
pub fn route_semantic(normalized: &str) -> Option<RouteDecision> {
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    if tokens.len() <= 3
        && let Some(intent) = tier1_map().get(normalized).copied()
    {
        return Some(RouteDecision::Tool(intent));
    }
    if conversational_help_suffix(normalized) {
        return Some(RouteDecision::Llm);
    }
    if looks_like_question(normalized) {
        return Some(RouteDecision::Llm);
    }
    try_embed_route(normalized)
}

struct EmbedState {
    matrix: Vec<(Intent, Vec<f32>)>,
}

static EMBED: Mutex<Option<EmbedState>> = Mutex::new(None);

fn model_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        out.push(local.join("Kuro").join("models").join("router-embed"));
    }
    out.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("bench")
            .join("models")
            .join("router-embed"),
    );
    out
}

pub fn find_onnx() -> Option<PathBuf> {
    if let Some(p) = kuro_core::router_embed_onnx() {
        return Some(p);
    }
    for dir in model_paths() {
        for name in ["model_quantized.onnx", "model_int8.onnx", "model.onnx", "bge-small-en-v1.5-int8.onnx"] {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn cache_path(hash: &str) -> PathBuf {
    kuro_core::KuroPaths::from_env().root.join(format!("router_exemplars_{hash}.bin"))
}

fn exemplar_hash() -> String {
    let mut h = Sha256::new();
    for ex in EXEMPLARS {
        h.update(format!("{:?}|{}", ex.intent, ex.text).as_bytes());
    }
    if let Ok(text) = std::fs::read_to_string(kuro_core::KuroPaths::from_env().workflows()) {
        h.update(text.as_bytes());
    }
    format!("{:x}", h.finalize())[..16].to_string()
}

fn try_load_cached_matrix(hash: &str) -> Option<Vec<(Intent, Vec<f32>)>> {
    let path = cache_path(hash);
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
    let dim = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
    if dim != EMBED_DIM {
        return None;
    }
    let mut offset = 8;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if offset + 4 + dim * 4 > bytes.len() {
            return None;
        }
        let intent_tag = u32::from_le_bytes(bytes[offset..offset + 4].try_into().ok()?);
        offset += 4;
        let intent = intent_from_tag(intent_tag)?;
        let mut row = vec![0.0f32; dim];
        for x in &mut row {
            *x = f32::from_le_bytes(bytes[offset..offset + 4].try_into().ok()?);
            offset += 4;
        }
        out.push((intent, row));
    }
    Some(out)
}

fn intent_tag(intent: Intent) -> u32 {
    match intent {
        Intent::Cancel => 0,
        Intent::Thanks => 1,
        Intent::Help => 2,
        Intent::Identity => 3,
        Intent::Time => 4,
        Intent::Date => 5,
        Intent::PauseMusic => 6,
        Intent::ResumeMusic => 7,
        Intent::StopMusic => 8,
        Intent::GoodNight => 9,
        Intent::DaddyHome => 10,
        Intent::PowerOff => 11,
        Intent::PowerOn => 12,
        Intent::GetNews => 13,
        Intent::MakeMeet => 14,
        Intent::CheckInbox => 15,
        Intent::SendWhatsapp => 16,
        Intent::RunWorkflow => 17,
        Intent::ReadScreen => 18,
        Intent::ConversationalLlm => 19,
    }
}

fn intent_from_tag(tag: u32) -> Option<Intent> {
    Some(match tag {
        0 => Intent::Cancel,
        1 => Intent::Thanks,
        2 => Intent::Help,
        3 => Intent::Identity,
        4 => Intent::Time,
        5 => Intent::Date,
        6 => Intent::PauseMusic,
        7 => Intent::ResumeMusic,
        8 => Intent::StopMusic,
        9 => Intent::GoodNight,
        10 => Intent::DaddyHome,
        11 => Intent::PowerOff,
        12 => Intent::PowerOn,
        13 => Intent::GetNews,
        14 => Intent::MakeMeet,
        15 => Intent::CheckInbox,
        16 => Intent::SendWhatsapp,
        17 => Intent::RunWorkflow,
        18 => Intent::ReadScreen,
        19 => Intent::ConversationalLlm,
        _ => return None,
    })
}

fn store_cached_matrix(hash: &str, matrix: &[(Intent, Vec<f32>)]) {
    let path = cache_path(hash);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(matrix.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(EMBED_DIM as u32).to_le_bytes());
    for (intent, row) in matrix {
        bytes.extend_from_slice(&intent_tag(*intent).to_le_bytes());
        for x in row {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
    }
    let _ = std::fs::write(path, bytes);
}

/// Deterministic bag-of-trigrams embedder for offline tests / weak fallback.
pub fn hash_embed(text: &str) -> [f32; EMBED_DIM] {
    let mut v = [0.0f32; EMBED_DIM];
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for (i, window) in bytes.windows(3).enumerate() {
        let mut h = 0u32;
        for b in window {
            h = h.wrapping_mul(16777619).wrapping_add(u32::from(*b));
        }
        let idx = (h as usize) % EMBED_DIM;
        v[idx] += 1.0 + (i % 7) as f32 * 0.01;
    }
    for (i, b) in bytes.iter().enumerate() {
        v[(*b as usize) % EMBED_DIM] += 0.15;
        v[(i * 13) % EMBED_DIM] += 0.05;
    }
    l2_normalize(&mut v);
    v
}

fn ensure_matrix() -> Option<()> {
    let mut slot = EMBED.lock().ok()?;
    if slot.is_some() {
        return Some(());
    }
    let hash = exemplar_hash();
    let cache_key = if crate::embed::ready() { format!("onnx_{hash}") } else { format!("hash_{hash}") };
    if let Some(matrix) = try_load_cached_matrix(&cache_key) {
        *slot = Some(EmbedState { matrix });
        return Some(());
    }
    let mut matrix = Vec::with_capacity(EXEMPLARS.len());
    if crate::embed::ready() {
        for ex in EXEMPLARS {
            let row = crate::embed::embed_text(ex.text).ok()?.to_vec();
            matrix.push((ex.intent, row));
        }
    } else {
        for ex in EXEMPLARS {
            matrix.push((ex.intent, hash_embed(ex.text).to_vec()));
        }
    }
    store_cached_matrix(&cache_key, &matrix);
    *slot = Some(EmbedState { matrix });
    Some(())
}

fn embed_enabled() -> bool {
    find_onnx().is_some()
        || matches!(
            std::env::var("KURO_ROUTER_EMBED").ok().as_deref().map(str::to_ascii_lowercase).as_deref(),
            Some("1" | "true" | "yes" | "on" | "hash")
        )
}

fn query_embed(normalized: &str) -> Option<Vec<f32>> {
    if crate::embed::ready() {
        crate::embed::embed_text(normalized).ok().map(|v| v.to_vec())
    } else {
        Some(hash_embed(normalized).to_vec())
    }
}

fn try_embed_route(normalized: &str) -> Option<RouteDecision> {
    if !embed_enabled() {
        return None;
    }
    ensure_matrix()?;
    let query = query_embed(normalized)?;
    let slot = EMBED.lock().ok()?;
    let state = slot.as_ref()?;
    match decide_from_matrix(&query, &state.matrix, normalized) {
        // Confident tool hit.
        RouteDecision::Tool(intent) => Some(RouteDecision::Tool(intent)),
        // Ambiguous / conversational → fall through to legacy exact/regex, then Ollama.
        RouteDecision::Llm => None,
    }
}

pub fn invalidate_exemplar_cache() {
    crate::embed::unload();
    if let Ok(mut slot) = EMBED.lock() {
        *slot = None;
    }
}

pub fn unload() {
    invalidate_exemplar_cache();
}

pub fn static_exemplars() -> Vec<(Intent, &'static str)> {
    EXEMPLARS.iter().map(|e| (e.intent, e.text)).collect()
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
