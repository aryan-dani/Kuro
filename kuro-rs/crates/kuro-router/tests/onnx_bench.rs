use std::time::Instant;

use kuro_router::embed;
use kuro_router::semantic::{EMBED_DIM, cosine_similarity_384, find_onnx, hash_embed, l2_normalize};

#[test]
fn measure_onnx_forward_and_avx2_dot() {
    if find_onnx().is_none() || !embed::ready() {
        eprintln!("router_onnx_forward_ms=not measured");
        return;
    }
    let _ = embed::embed_text("warmup desk on").expect("warmup");
    let n = 30;
    let start = Instant::now();
    for i in 0..n {
        let _ = embed::embed_text(&format!("turn on the desk lights please {i}")).expect("embed");
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
    eprintln!("router_onnx_forward_ms={ms:.3}");

    let mut a = hash_embed("desk on");
    let mut b = hash_embed("turn on the desk");
    l2_normalize(&mut a);
    l2_normalize(&mut b);
    let aa = a;
    let mut bb = [0.0f32; EMBED_DIM];
    bb.copy_from_slice(&b);

    let iters = 500_000u32;
    let start = Instant::now();
    let mut acc = 0.0f32;
    for _ in 0..iters {
        acc += cosine_similarity_384(&aa, &bb);
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / f64::from(iters);
    let ns = us * 1000.0;
    eprintln!("avx2_dot_us={us:.4} avx2_dot_ns={ns:.2} acc={acc}");
    assert!(acc.is_finite());
}
