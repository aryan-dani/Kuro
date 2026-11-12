//! Shared ONNX Runtime environment + session helpers (workspace-pinned `ort` 2.0.0-rc.15).

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use ort::environment::Environment;
use ort::ep::CPU;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;

static ENV: OnceLock<Environment> = OnceLock::new();
static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn environment() -> Result<&'static Environment, String> {
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let _guard = ENV_LOCK.lock().map_err(|_| "ort env lock".to_string())?;
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = ort::init()
        .with_name("kuro")
        .with_execution_providers([CPU::default().with_arena_allocator(true).build()])
        .build()
        .map_err(|e| format!("ort init: {e}"))?;
    let _ = ENV.set(env);
    ENV.get().ok_or_else(|| "ort env missing after init".to_string())
}

pub fn ensure_init() {
    let _ = environment();
}

pub fn shutdown() {
    // Sessions own their graphs; dropping them releases arenas. Global Environment stays for process life.
}

pub fn session_from_file(path: &Path) -> Result<Session, String> {
    session_from_file_with_threads(path, 2)
}

pub fn session_from_file_with_threads(path: &Path, intra_threads: usize) -> Result<Session, String> {
    let env = environment()?;
    let builder = Session::builder(env).map_err(|e| format!("ort builder: {e}"))?;
    let builder =
        builder.with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| format!("ort opt: {e}"))?;
    let builder = builder.with_intra_threads(intra_threads).map_err(|e| format!("ort threads: {e}"))?;
    let mut builder = builder.with_memory_pattern(true).map_err(|e| format!("ort mem pattern: {e}"))?;
    builder.commit_from_file(path).map_err(|e| format!("ort load {}: {e}", path.display()))
}
