//! Prove dormant unload drops RSS after Whisper/Kokoro/router were mapped.

use std::time::Duration;

#[test]
fn working_set_after_unload_below_15mb() {
    if kuro_stt::ggml_path().is_none() {
        eprintln!("skip: Whisper GGML absent");
        return;
    }
    // Load heavy models.
    let _ = kuro_stt::prewarm();
    if kuro_tts::kokoro_models_present() {
        let _ = kuro_tts::synthesize("Desk on.");
    }
    if kuro_router::semantic::find_onnx().is_some() {
        let _ = kuro_router::route_semantic("desk on");
        // Force exemplar matrix build via a non-tier1 utterance when embed is live.
        let _ = kuro_router::try_handle("create a google meet link please");
    }
    let peak = kuro_core::working_set_bytes().unwrap_or(0);
    eprintln!("peak_ws_mb={:.1}", peak as f64 / (1024.0 * 1024.0));

    kuro_router::semantic::unload();
    kuro_stt::shutdown_worker();
    kuro_tts::shutdown_worker();
    kuro_core::ort_env::shutdown();
    kuro_core::trim_working_set();
    std::thread::sleep(Duration::from_millis(200));
    kuro_core::trim_working_set();
    std::thread::sleep(Duration::from_millis(200));

    let after = kuro_core::working_set_bytes().unwrap_or(u64::MAX);
    let mb = after as f64 / (1024.0 * 1024.0);
    eprintln!("after_unload_ws_mb={mb:.1}");
    assert!(mb < 15.0, "working set after unload was {mb:.1} MB (want < 15)");
}
