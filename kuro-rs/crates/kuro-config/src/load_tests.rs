use super::*;
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn new() -> Self {
        Self { _lock: ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner()), saved: Vec::new() }
    }

    fn set(&mut self, key: &str, value: &str) {
        self.save(key);
        // Safety: test-only env mutation.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove(&mut self, key: &str) {
        self.save(key);
        // Safety: test-only env mutation.
        unsafe { std::env::remove_var(key) };
    }

    fn save(&mut self, key: &str) {
        if self.saved.iter().any(|(k, _)| k == key) {
            return;
        }
        let prev = std::env::var(key).ok();
        self.saved.push((key.to_string(), prev));
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, prev) in self.saved.drain(..) {
            match prev {
                Some(v) => unsafe { std::env::set_var(&key, v) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
    }
}

fn load_from_profile(path: &Path) -> Result<Settings, ConfigLoadError> {
    let mut settings = Settings::default();
    if path.is_file() {
        apply_profile_file(&mut settings, path)?;
    }
    apply_env(&mut settings);
    Ok(settings)
}

#[test]
fn defaults_match_engine() {
    let mut guard = EnvGuard::new();
    for key in [
        "KURO_LLM_MODE",
        "KURO_STT_BACKEND",
        "KURO_TTS_ENGINE",
        "KURO_API_PORT",
        "KURO_HUB_URL",
        "KURO_HUB_TOKEN",
        "KURO_N8N_BASE_URL",
        "KURO_N8N_TOKEN",
        "KURO_SAY_EXECUTE",
        "KURO_DATA_DIR",
        "KURO_WAKE_WORD",
        "KURO_WAKE_WORD_MODEL",
        "KURO_LISTENER",
    ] {
        guard.remove(key);
    }
    let dir = std::env::temp_dir().join(format!("kuro-cfg-def-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let missing = dir.join("missing.toml");
    let s = load_from_profile(&missing).unwrap();
    assert_eq!(s, Settings::default());
}

#[test]
fn profile_overrides_default() {
    let mut guard = EnvGuard::new();
    guard.remove("KURO_LLM_MODE");
    let dir = std::env::temp_dir().join(format!("kuro-cfg-prof-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "llm_mode = \"off\"\nstt_backend = \"cpu\"\n").unwrap();
    let s = load_from_profile(&path).unwrap();
    assert_eq!(s.llm_mode, "off");
    assert_eq!(s.stt_backend, "cpu");
    assert_eq!(s.tts_engine, "auto");
}

#[test]
fn env_overrides_profile() {
    let mut guard = EnvGuard::new();
    guard.remove("KURO_LLM_MODE");
    let dir = std::env::temp_dir().join(format!("kuro-cfg-env-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "llm_mode = \"off\"\n").unwrap();
    guard.set("KURO_LLM_MODE", "always");
    let s = load_from_profile(&path).unwrap();
    assert_eq!(s.llm_mode, "always");
}

#[test]
fn missing_profile_is_ok() {
    let dir = std::env::temp_dir().join(format!("kuro-cfg-miss-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("nope.toml");
    assert!(load_from_profile(&path).is_ok());
}

#[test]
fn invalid_toml_is_typed_error() {
    let dir = std::env::temp_dir().join(format!("kuro-cfg-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "llm_mode = \n").unwrap();
    let err = load_from_profile(&path).unwrap_err();
    assert!(matches!(err, ConfigLoadError::ProfileParse(_)));
}

#[test]
fn wake_word_disabled_by_default() {
    assert!(!Settings::default().wake_word_enabled);
}

#[test]
fn env_enables_wake_word() {
    let mut guard = EnvGuard::new();
    guard.remove("KURO_WAKE_WORD");
    guard.remove("KURO_WAKE_WORD_MODEL");
    let dir = std::env::temp_dir().join(format!("kuro-cfg-wake-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    guard.set("KURO_WAKE_WORD", "1");
    guard.set("KURO_WAKE_WORD_MODEL", "models/kuro.onnx");
    let s = load_from_profile(&path).unwrap();
    assert!(s.wake_word_enabled);
    assert_eq!(s.wake_word_model.as_deref().and_then(|p| p.to_str()), Some("models/kuro.onnx"));
}

#[test]
fn profile_wake_word_section() {
    let mut guard = EnvGuard::new();
    guard.remove("KURO_WAKE_WORD");
    let dir = std::env::temp_dir().join(format!("kuro-cfg-wake-prof-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[wake_word]\nenabled = true\nmodel = \"wake/kuro.tflite\"\n").unwrap();
    let s = load_from_profile(&path).unwrap();
    assert!(s.wake_word_enabled);
    assert_eq!(s.wake_word_model.as_deref().and_then(|p| p.to_str()), Some("wake/kuro.tflite"));
}

#[test]
fn listener_disabled_by_default() {
    assert!(!Settings::default().listener_enabled);
}

#[test]
fn env_enables_listener() {
    let mut guard = EnvGuard::new();
    guard.remove("KURO_LISTENER");
    let dir = std::env::temp_dir().join(format!("kuro-cfg-listen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    guard.set("KURO_LISTENER", "1");
    let s = load_from_profile(&path).unwrap();
    assert!(s.listener_enabled);
}

#[test]
fn profile_listener_section() {
    let mut guard = EnvGuard::new();
    guard.remove("KURO_LISTENER");
    let dir = std::env::temp_dir().join(format!("kuro-cfg-listen-prof-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[listener]\nenabled = true\n").unwrap();
    let s = load_from_profile(&path).unwrap();
    assert!(s.listener_enabled);
}
