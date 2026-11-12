use kuro_tts::{Speaker, TtsKind, kokoro_models_present, select_kind};

#[test]
fn auto_and_piper_share_the_native_engine() {
    let auto = if kokoro_models_present() { TtsKind::Kokoro } else { TtsKind::Piper };
    assert_eq!(select_kind("auto"), auto);
    assert_eq!(select_kind("kokoro"), TtsKind::Kokoro);
    assert_eq!(select_kind("piper"), TtsKind::Piper);
    assert_eq!(select_kind("pyttsx3"), TtsKind::Sapi);
    assert_eq!(select_kind("xtts"), TtsKind::Xtts);
}

#[test]
fn clip_cache_synthesizes_once() {
    let mut speaker = Speaker::default();
    let mut starts = 0;
    let alpha = "Gate cache alpha phrase.";
    let beta = "Gate cache beta phrase.";
    assert!(speaker.speak(alpha, || true, || starts += 1));
    assert!(speaker.speak(beta, || true, || starts += 1));
    assert!(speaker.speak(alpha, || true, || starts += 1));
    assert_eq!(speaker.synth_calls(), 2);
    assert_eq!(starts, 3);
    assert!(speaker.cached(alpha));
}

#[test]
fn stale_epoch_does_not_start_audio() {
    let mut speaker = Speaker::default();
    let mut starts = 0;
    assert!(!speaker.speak("Anytime.", || false, || starts += 1));
    assert_eq!(starts, 0);
    assert_eq!(speaker.synth_calls(), 0);
}
