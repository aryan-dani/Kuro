use std::path::PathBuf;
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use super::decide::decide_from_matrix;
use super::exemplars::EXEMPLARS;
use super::similarity::{EMBED_DIM, l2_normalize};
use super::{Intent, RouteDecision};

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

pub(crate) fn try_embed_route(normalized: &str) -> Option<RouteDecision> {
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
