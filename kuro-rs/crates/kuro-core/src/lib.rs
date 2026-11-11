//! Shared paths, turn epoch, timing, and engine config.

mod close;
mod config;
mod epoch;
mod paths;
mod timing;
mod token;
mod voice;

pub use close::{get_close_matches, ratio};
pub use config::{ConfigError, EngineConfig, apply_config, mask_id, mask_number, public_config};
pub use epoch::{FollowUp, HudState, TurnClock, follow_up_for, prefer_transcript};
pub use paths::KuroPaths;
pub use timing::Timing;
pub use token::{api_port, api_requested, ensure_token, load_token, token_matches};
pub use voice::voice_level;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn local_root() -> PathBuf {
    if let Ok(dir) = std::env::var("KURO_DATA_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("Kuro")
}

pub fn append_log(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Rotate by size: when `kuro.log` exceeds `max_bytes`, rename to `kuro.log.1`.
pub fn append_rotating_log(path: &Path, line: &str, max_bytes: u64) {
    if path.is_file()
        && let Ok(meta) = fs::metadata(path)
        && meta.len() > max_bytes
    {
        let backup = path.with_extension("log.1");
        let _ = fs::rename(path, backup);
    }
    append_log(path, line);
}

pub fn write_heartbeat(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = fs::write(path, format!("{stamp}\n"));
}

/// Exclusive lock file. Returns false when another engine already holds it.
pub fn try_singleton(lock_path: &Path) -> bool {
    if let Some(parent) = lock_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match OpenOptions::new().write(true).create_new(true).open(lock_path) {
        Ok(mut file) => {
            let _ = writeln!(file, "{}", std::process::id());
            true
        }
        Err(_) => false,
    }
}

pub fn release_singleton(lock_path: &Path) {
    let _ = fs::remove_file(lock_path);
}
