//! STT bias and contact fuzzy match for Whisper near-misses (sia/zia → Siya).

use std::collections::HashMap;

use kuro_core::get_close_matches;

use crate::whatsapp::normalize;

const CUTOFF: f64 = 0.86;

/// Contact display names for Whisper initial_prompt (short book first).
pub fn stt_contact_names(book: &HashMap<String, String>) -> Vec<String> {
    let mut names: Vec<String> = book
        .keys()
        .filter(|n| {
            let key = normalize(n);
            !key.is_empty() && key != "me" && key.len() <= 24
        })
        .cloned()
        .collect();
    names.sort_by_key(|n| normalize(n));
    names.truncate(12);
    names
}

/// Whisper often swaps s/z and drops a vowel ("Siya" → "Sia" / "Zia").
pub fn queries_to_try(query_n: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(4);
    if query_n.is_empty() {
        return out;
    }
    out.push(query_n.to_string());
    let bytes = query_n.as_bytes();
    if bytes[0] == b's' {
        out.push(format!("z{}", &query_n[1..]));
    } else if bytes[0] == b'z' {
        out.push(format!("s{}", &query_n[1..]));
    }
    out.sort();
    out.dedup();
    out
}

fn edit_distance(a: &str, b: &str) -> usize {
    if a.len().abs_diff(b.len()) > 1 {
        return 2;
    }
    if a.len() == b.len() {
        return a.bytes().zip(b.bytes()).filter(|(x, y)| x != y).count();
    }
    let (longer, shorter) = if a.len() > b.len() { (a, b) } else { (b, a) };
    let lb = longer.as_bytes();
    let sb = shorter.as_bytes();
    let mut i = 0;
    let mut j = 0;
    let mut skipped = 0;
    while i < lb.len() && j < sb.len() {
        if lb[i] != sb[j] {
            skipped += 1;
            if skipped > 1 {
                return 2;
            }
            i += 1;
            continue;
        }
        i += 1;
        j += 1;
    }
    1
}

fn shared_prefix(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

fn one_letter_miss(query_n: &str, book: &HashMap<String, String>) -> Option<(String, String)> {
    let mut found: HashMap<String, (String, String)> = HashMap::new();
    for (name, digits) in book {
        let key = normalize(name);
        if !key.is_empty() && edit_distance(query_n, &key) == 1 {
            found.entry(key).or_insert_with(|| (name.clone(), digits.clone()));
        }
    }
    if found.is_empty() {
        return None;
    }
    if found.len() == 1 {
        return found.into_values().next();
    }
    let mut ranked: Vec<(String, (String, String))> = found.into_iter().collect();
    ranked.sort_by_key(|pair| std::cmp::Reverse(shared_prefix(query_n, &pair.0)));
    let best_n = shared_prefix(query_n, &ranked[0].0);
    let next_n = shared_prefix(query_n, &ranked[1].0);
    if best_n >= 2 && best_n > next_n {
        return Some(ranked.remove(0).1);
    }
    None
}

fn resolve_one(query_n: &str, book: &HashMap<String, String>) -> Option<(String, String)> {
    for (name, digits) in book {
        if normalize(name) == query_n {
            return Some((name.clone(), digits.clone()));
        }
    }
    let choices: Vec<&str> = book.keys().map(|s| s.as_str()).collect();
    if let Some(hit) = get_close_matches(query_n, &choices, 1, CUTOFF).into_iter().next() {
        return book.get(hit).map(|d| (hit.to_string(), d.clone()));
    }
    let norms: Vec<String> = choices.iter().map(|n| normalize(n)).collect();
    let refs: Vec<&str> = norms.iter().map(|s| s.as_str()).collect();
    if let Some(hit) = get_close_matches(query_n, &refs, 1, CUTOFF).into_iter().next() {
        for (name, digits) in book {
            if normalize(name) == hit {
                return Some((name.clone(), digits.clone()));
            }
        }
    }
    one_letter_miss(query_n, book)
}

pub fn resolve_contact(query: &str, book: &HashMap<String, String>) -> Option<(String, String)> {
    let query_n = normalize(query);
    if query_n.is_empty() || book.is_empty() {
        return None;
    }
    for variant in queries_to_try(&query_n) {
        if let Some(hit) = resolve_one(&variant, book) {
            return Some(hit);
        }
    }
    None
}

/// Rewrite heard tokens to book display names before routing (Sia → Siya).
pub fn rewrite_transcript(text: &str, book: &HashMap<String, String>) -> String {
    if text.trim().is_empty() || book.is_empty() {
        return text.to_string();
    }
    text.split_whitespace()
        .map(|tok| {
            let clean = tok.trim_matches(|c: char| !c.is_alphanumeric());
            if clean.len() < 2 {
                return tok.to_string();
            }
            match resolve_contact(clean, book) {
                Some((name, _)) if normalize(&name) != normalize(clean) => {
                    if clean.len() == tok.len() {
                        name
                    } else {
                        tok.replacen(clean, &name, 1)
                    }
                }
                _ => tok.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> HashMap<String, String> {
        [("Siya".into(), "910000000001".into()), ("Mom".into(), "910000000002".into())].into_iter().collect()
    }

    #[test]
    fn sia_and_zia_resolve_to_siya() {
        let b = book();
        assert_eq!(resolve_contact("sia", &b).unwrap().0, "Siya");
        assert_eq!(resolve_contact("Sia", &b).unwrap().0, "Siya");
        assert_eq!(resolve_contact("zia", &b).unwrap().0, "Siya");
        assert_eq!(resolve_contact("Zia", &b).unwrap().0, "Siya");
    }

    #[test]
    fn rewrite_swaps_sia_token() {
        let b = book();
        let out = rewrite_transcript("send a google meet link to Sia", &b);
        assert!(out.to_ascii_lowercase().contains("siya"), "got: {out}");
        assert!(!out.split_whitespace().any(|t| t.eq_ignore_ascii_case("sia")));
    }

    #[test]
    fn stt_names_include_siya() {
        let names = stt_contact_names(&book());
        assert!(names.iter().any(|n| n == "Siya"));
    }
}
