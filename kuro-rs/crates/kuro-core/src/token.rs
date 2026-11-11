use std::fs;
use std::path::Path;

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
    Ok(load_token(path))
}

fn random_hex(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes * 2);
    let mut state =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
    state ^= std::process::id() as u128;
    for _ in 0..bytes {
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
