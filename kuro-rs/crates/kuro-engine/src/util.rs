pub fn gate_ci_mode() -> bool {
    matches!(
        std::env::var("KURO_GATE_CI").unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub fn log_line(msg: &str) {
    let paths = kuro_core::KuroPaths::from_env();
    kuro_core::append_rotating_log(&paths.log(), msg, 1_000_000);
}
