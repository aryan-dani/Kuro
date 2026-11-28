pub fn unload_speech() {
    kuro_router::semantic::unload();
    kuro_stt::shutdown_worker();
    kuro_tts::shutdown_worker();
    kuro_core::ort_env::shutdown();
    kuro_core::trim_working_set();
}

pub fn warm_speech() {
    let _ = kuro_stt::prewarm();
    let _ = kuro_tts::prewarm();
    kuro_tts::prebuild_phrase_cache();
    if ollama_up() {
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:e4b".into());
        kuro_tools::brain::prewarm_model(&model);
    }
}

/// Load the chat model without touching speech workers. Runs at boot even away from home,
/// so the first planned request does not wait for a cold model load.
pub fn warm_ollama() {
    if ollama_up() {
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma4:e4b".into());
        kuro_tools::brain::prewarm_model(&model);
    }
}

pub fn ollama_up() -> bool {
    let Ok(addr) = "127.0.0.1:11434".parse() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok()
}
