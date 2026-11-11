use std::collections::HashMap;

/// Ratcliff/Obershelp ratio, matching `difflib.SequenceMatcher.ratio` for short strings.
pub fn ratio(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let matches = matching_size(a.as_bytes(), b.as_bytes());
    2.0 * matches as f64 / (a.len() + b.len()) as f64
}

fn matching_size(a: &[u8], b: &[u8]) -> usize {
    fn rec(a: &[u8], b: &[u8], alo: usize, ahi: usize, blo: usize, bhi: usize, acc: &mut usize) {
        let (i, j, k) = longest(a, b, alo, ahi, blo, bhi);
        if k == 0 {
            return;
        }
        rec(a, b, alo, i, blo, j, acc);
        *acc += k;
        rec(a, b, i + k, ahi, j + k, bhi, acc);
    }
    let mut acc = 0;
    rec(a, b, 0, a.len(), 0, b.len(), &mut acc);
    acc
}

fn longest(a: &[u8], b: &[u8], alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
    let mut b2j: HashMap<u8, Vec<usize>> = HashMap::new();
    for (j, byte) in b.iter().enumerate().take(bhi).skip(blo) {
        b2j.entry(*byte).or_default().push(j);
    }
    let mut j2len: HashMap<usize, usize> = HashMap::new();
    let mut best_i = alo;
    let mut best_j = blo;
    let mut best = 0usize;
    for (i, byte) in a.iter().enumerate().take(ahi).skip(alo) {
        let mut next = HashMap::new();
        if let Some(js) = b2j.get(byte) {
            for &j in js {
                let prev = if j > 0 { j2len.get(&(j - 1)).copied().unwrap_or(0) } else { 0 };
                let k = prev + 1;
                next.insert(j, k);
                if k > best {
                    best = k;
                    best_i = i + 1 - k;
                    best_j = j + 1 - k;
                }
            }
        }
        j2len = next;
    }
    (best_i, best_j, best)
}

pub fn get_close_matches<'a>(word: &str, possibilities: &[&'a str], n: usize, cutoff: f64) -> Vec<&'a str> {
    let mut scored: Vec<(f64, &'a str)> = Vec::new();
    for item in possibilities {
        let score = ratio(word, item);
        if score >= cutoff {
            scored.push((score, *item));
        }
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(n).map(|(_, s)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_matches_python_fixtures() {
        let score = ratio("morning breif", "morning brief");
        assert!(score > 0.9, "{score}");
        assert!(ratio("banana radio", "morning brief") < 0.75);
    }
}
