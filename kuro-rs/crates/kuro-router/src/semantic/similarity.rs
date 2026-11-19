pub const EMBED_DIM: usize = 384;

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
