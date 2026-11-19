use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

pub fn load_token(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default().trim_start_matches('\u{feff}').trim().to_string()
}

pub fn token_matches(expected: &str, presented: &str) -> bool {
    let presented = presented.trim().trim_start_matches('\u{feff}');
    if expected.is_empty() || presented.is_empty() || presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in expected.bytes().zip(presented.bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn ensure_token(path: &Path) -> std::io::Result<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.is_file() {
        let token = random_hex(32);
        fs::write(path, &token)?;
        return Ok(token);
    }
    let existing = load_token(path);
    if is_weak_token(&existing) {
        let token = random_hex(32);
        fs::write(path, &token)?;
        return Ok(token);
    }
    Ok(existing)
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).expect("OS random bytes");
    let mut out = String::with_capacity(bytes * 2);
    for byte in buf {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn weak_token_catalog() -> &'static HashSet<String> {
    static WEAK: OnceLock<HashSet<String>> = OnceLock::new();
    WEAK.get_or_init(|| (0u8..=255).map(lcg_random_hex_low_seed).collect())
}

pub(crate) fn is_weak_token(token: &str) -> bool {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    if token.bytes().any(|b| b.is_ascii_uppercase()) {
        return false;
    }
    weak_token_catalog().contains(token)
}

fn lcg_random_hex_low_seed(low: u8) -> String {
    let mut out = String::with_capacity(64);
    let mut state = u128::from(low);
    for _ in 0..32 {
        state = state.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(0x6A09_E667);
        out.push_str(&format!("{:02x}", (state & 0xff) as u8));
    }
    out
}

pub fn api_requested() -> bool {
    matches!(std::env::var("KURO_API").unwrap_or_default().to_ascii_lowercase().as_str(), "on" | "1" | "true" | "yes")
}

pub fn api_port() -> u16 {
    std::env::var("KURO_API_PORT")
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .filter(|p| (1..=65535).contains(p))
        .unwrap_or(8765)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_long() {
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let token = random_hex(32);
            assert_eq!(token.len(), 64);
            assert!(token.bytes().all(|b| matches!(b, b'a'..=b'f' | b'0'..=b'9')));
            assert!(seen.insert(token));
        }
    }

    #[test]
    fn weak_token_catalog_covers_low_seeds() {
        for low in 0u8..=255 {
            let token = lcg_random_hex_low_seed(low);
            assert!(is_weak_token(&token), "expected weak token for seed {low}");
        }
        for _ in 0..1000 {
            let token = random_hex(32);
            assert!(!is_weak_token(&token));
        }
    }

    #[test]
    fn weak_token_is_replaced() {
        let dir = std::env::temp_dir().join(format!("kuro-token-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("api_token");
        let strong = random_hex(32);
        std::fs::write(&path, &strong).unwrap();
        assert_eq!(ensure_token(&path).unwrap(), strong);

        let weak = lcg_random_hex_low_seed(42);
        std::fs::write(&path, &weak).unwrap();
        let rotated = ensure_token(&path).unwrap();
        assert_ne!(rotated, weak);
        assert!(!is_weak_token(&rotated));
        assert_eq!(load_token(&path), rotated);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
