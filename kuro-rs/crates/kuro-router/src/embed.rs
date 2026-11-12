//! Live ONNX sentence embeddings via workspace-pinned `ort`.

use std::sync::Mutex;

use ort::value::Tensor;

use crate::tokenizer;

pub const EMBED_DIM: usize = 384;

struct EmbedSession {
    session: ort::session::Session,
}

static SESSION: Mutex<Option<EmbedSession>> = Mutex::new(None);

pub fn unload() {
    tokenizer::unload();
    if let Ok(mut slot) = SESSION.lock() {
        *slot = None;
    }
}

pub fn ready() -> bool {
    kuro_core::router_embed_onnx().is_some() && kuro_core::router_embed_vocab().is_some()
}

fn ensure() -> Result<(), String> {
    let mut slot = SESSION.lock().map_err(|_| "embed lock".to_string())?;
    if slot.is_some() {
        return Ok(());
    }
    let path = kuro_core::router_embed_onnx().ok_or_else(|| "router-embed ONNX missing".to_string())?;
    tokenizer::ensure_tokenizer().ok_or_else(|| "router-embed vocab missing".to_string())?;
    let mut session = kuro_core::ort_env::session_from_file_with_threads(&path, 4)?;
    // Warm up ONNX Runtime memory pattern and execution kernels on a 4-token dummy pass.
    let dummy_ids = Tensor::from_array(([1usize, 4usize], vec![101i64, 2054, 2003, 102])).map_err(|e| e.to_string())?;
    let dummy_mask = Tensor::from_array(([1usize, 4usize], vec![1i64; 4])).map_err(|e| e.to_string())?;
    let dummy_types = Tensor::from_array(([1usize, 4usize], vec![0i64; 4])).map_err(|e| e.to_string())?;
    let _ = session
        .run(ort::inputs![
            "input_ids" => dummy_ids,
            "attention_mask" => dummy_mask,
            "token_type_ids" => dummy_types,
        ])
        .map_err(|e| format!("embed warmup: {e}"))?;
    *slot = Some(EmbedSession { session });
    Ok(())
}

/// Public for benches / integration tests.
pub fn embed_text(text: &str) -> Result<[f32; EMBED_DIM], String> {
    ensure()?;
    let (ids, mask, types) = tokenizer::encode(text).ok_or_else(|| "tokenize failed".to_string())?;
    let seq_len = ids.len().max(2);
    let mut slot = SESSION.lock().map_err(|_| "embed lock".to_string())?;
    let sess = slot.as_mut().ok_or_else(|| "embed session missing".to_string())?;

    let input_ids = Tensor::from_array(([1usize, seq_len], ids)).map_err(|e| e.to_string())?;
    let attention_mask = Tensor::from_array(([1usize, seq_len], mask)).map_err(|e| e.to_string())?;
    let token_type_ids = Tensor::from_array(([1usize, seq_len], types)).map_err(|e| e.to_string())?;

    let outputs = sess
        .session
        .run(ort::inputs![
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        ])
        .map_err(|e| format!("embed forward: {e}"))?;

    // Prefer last_hidden_state; fall back to first output.
    let (_name, value) = outputs
        .iter()
        .find(|(n, _)| {
            let n: &str = n;
            n == "last_hidden_state"
        })
        .or_else(|| outputs.iter().next())
        .ok_or_else(|| "embed: empty outputs".to_string())?;

    let (shape, data) = value.try_extract_tensor::<f32>().map_err(|e| format!("embed extract: {e}"))?;
    // shape [1, seq, 384]
    if shape.len() != 3 || shape[2] as usize != EMBED_DIM {
        return Err(format!("unexpected embed shape {shape:?}"));
    }
    let seq = shape[1] as usize;
    let mut pooled = [0.0f32; EMBED_DIM];
    for t in 0..seq {
        let base = t * EMBED_DIM;
        for d in 0..EMBED_DIM {
            pooled[d] += data[base + d];
        }
    }
    let count = (seq as f32).max(1.0);
    for x in &mut pooled {
        *x /= count;
    }
    crate::semantic::l2_normalize(&mut pooled);
    Ok(pooled)
}
