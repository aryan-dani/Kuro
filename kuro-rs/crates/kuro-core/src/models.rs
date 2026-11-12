//! Resolve ONNX model directories under LOCALAPPDATA and bench/models.

use std::path::{Path, PathBuf};

pub fn repo_root_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(root) = std::env::var("KURO_ROOT") {
        let p = PathBuf::from(root.trim());
        if p.is_dir() {
            out.push(p);
        }
    }
    // kuro-core crate → kuro-rs → repo
    let from_core = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..");
    if let Ok(canon) = std::fs::canonicalize(&from_core) {
        out.push(canon);
    } else {
        out.push(from_core);
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.clone());
        out.push(cwd.join(".."));
    }
    out
}

/// `%LOCALAPPDATA%\Kuro\models\{name}` then `bench/models/{name}` under repo roots.
pub fn resolve_model_dir(name: &str) -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var(format!("KURO_MODEL_DIR_{}", name.to_ascii_uppercase().replace('-', "_"))) {
        let p = PathBuf::from(override_dir.trim());
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let candidate = local.join("Kuro").join("models").join(name);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    for root in repo_root_candidates() {
        let candidate = root.join("bench").join("models").join(name);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

pub fn first_existing(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn router_embed_onnx() -> Option<PathBuf> {
    let dir = resolve_model_dir("router-embed")?;
    first_existing(&dir, &["model_quantized.onnx", "model_int8.onnx", "model.onnx", "bge-small-en-v1.5-int8.onnx"])
}

pub fn router_embed_vocab() -> Option<PathBuf> {
    let dir = resolve_model_dir("router-embed")?;
    first_existing(&dir, &["vocab.txt", "tokenizer.json"])
}

pub fn kokoro_dir() -> Option<PathBuf> {
    resolve_model_dir("kokoro")
}

pub fn silero_vad_onnx() -> Option<PathBuf> {
    let dir = resolve_model_dir("silero-vad")?;
    first_existing(&dir, &["silero_vad.onnx", "model.onnx"])
}
