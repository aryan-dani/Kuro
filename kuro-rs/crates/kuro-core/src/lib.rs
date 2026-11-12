//! Shared paths, turn epoch, timing, and engine config.

mod close;
mod config;
mod epoch;
mod mem;
pub mod models;
mod paths;
mod timing;
mod token;
mod voice;

pub use close::{get_close_matches, jaro, jaro_winkler, ratio};
pub use mem::{trim_working_set, working_set_bytes};
pub mod ort_env;
pub use config::{ConfigError, EngineConfig, apply_config, mask_id, mask_number, public_config};
pub use epoch::{FollowUp, HudState, TurnClock, follow_up_for, prefer_transcript};
pub use models::{kokoro_dir, resolve_model_dir, router_embed_onnx, router_embed_vocab, silero_vad_onnx};
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

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        let _ = CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE as u32
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    pid != 0 && Path::new("/proc").join(pid.to_string()).exists()
}

fn lock_holder_dead(lock_path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(lock_path) else {
        return false;
    };
    let Ok(pid) = text.trim().parse::<u32>() else {
        return false;
    };
    !pid_alive(pid)
}

/// Exclusive lock file. Returns false when another live engine already holds it.
pub fn try_singleton(lock_path: &Path) -> bool {
    if let Some(parent) = lock_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    for attempt in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(lock_path) {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", std::process::id());
                return true;
            }
            Err(err)
                if attempt == 0 && err.kind() == std::io::ErrorKind::AlreadyExists && lock_holder_dead(lock_path) =>
            {
                let _ = fs::remove_file(lock_path);
            }
            Err(_) => return false,
        }
    }
    false
}

pub fn release_singleton(lock_path: &Path) {
    let _ = fs::remove_file(lock_path);
}

/// Load selected keys from a `.env` file into the process without overwriting existing values.
/// Never logs secret values.
pub fn load_env_file(path: &Path, keys: &[&str]) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_key, raw_val)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        if !keys.contains(&key) {
            continue;
        }
        if std::env::var_os(key).is_some_and(|v| !v.is_empty()) {
            continue;
        }
        let mut val = raw_val.trim().to_string();
        if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\'')) {
            val = val[1..val.len().saturating_sub(1)].to_string();
        }
        if !val.is_empty() {
            // Safety: process-local env for this engine only.
            unsafe { std::env::set_var(key, val) };
        }
    }
}

/// Repo root for tool Python (KURO_ROOT, else climb from CARGO_MANIFEST when building tools).
pub fn repo_root_hint() -> PathBuf {
    if let Ok(root) = std::env::var("KURO_ROOT") {
        let path = PathBuf::from(root.trim());
        if path.is_dir() {
            return path;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
