use std::path::Path;

pub fn log_loaded_settings(log_path: &Path) {
    let note = kuro_config::Settings::load()
        .map(|s| format!("config llm_mode={}", s.llm_mode))
        .unwrap_or_else(|e| format!("config load error: {e}"));
    kuro_core::append_rotating_log(log_path, &note, 1_000_000);
}
