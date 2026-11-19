//! Map `kuro-triggers` activations into turn events without touching the PTT hook.

use kuro_config::Settings;
use kuro_triggers::{NullWakeWord, TriggerRegistry};

#[cfg(test)]
use kuro_triggers::FakeDetector;
use kuro_turn::Event;

/// Build the wake-word side of the trigger registry. Hold-to-talk stays in `kuro-hotkey`.
pub fn build_wake_registry(settings: &Settings) -> TriggerRegistry {
    let mut registry = TriggerRegistry::new();
    if settings.wake_word_enabled {
        // Licensed local model adapter lands after the phase-25 spike; stub stays silent.
        registry.register(Box::new(NullWakeWord));
    } else {
        registry.register(Box::new(NullWakeWord));
    }
    registry
}

/// True when wake word is on and a model path is configured (open-mic adapter is later).
pub fn wake_would_open_mic(settings: &Settings) -> bool {
    settings.wake_word_enabled && settings.wake_word_model.is_some()
}

pub fn log_wake_boot(settings: &Settings) {
    if settings.wake_word_enabled {
        let model = settings.wake_word_model.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "none".into());
        crate::log_line(&format!("wake_word on model={model} (stub detector, no open mic yet)"));
    } else {
        crate::log_line("wake_word off");
    }
}

pub fn activation_to_turn_event(source_id: &str) -> Option<Event> {
    if source_id == "fake.wake" || source_id.starts_with("wake_word.") {
        return Some(Event::HotkeyDown);
    }
    None
}

/// Poll registered wake triggers once; wake activations become `HotkeyDown` for the turn machine.
pub fn poll_wake_turn_event(registry: &mut TriggerRegistry) -> Option<Event> {
    registry.poll_all().into_iter().find_map(|a| activation_to_turn_event(&a.source_id))
}

#[cfg(test)]
pub fn build_fake_wake_registry_for_test() -> TriggerRegistry {
    let mut registry = TriggerRegistry::new();
    registry.register(Box::new(FakeDetector::once("fake.wake")));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_registry_never_emits_turn_event() {
        let settings = Settings::default();
        assert!(!settings.wake_word_enabled);
        let mut registry = build_wake_registry(&settings);
        assert!(poll_wake_turn_event(&mut registry).is_none());
        assert!(poll_wake_turn_event(&mut registry).is_none());
    }

    #[test]
    fn fake_detector_maps_to_hotkey_down_once() {
        let mut registry = build_fake_wake_registry_for_test();
        assert_eq!(poll_wake_turn_event(&mut registry), Some(Event::HotkeyDown));
        assert!(poll_wake_turn_event(&mut registry).is_none());
    }

    #[test]
    fn wake_would_open_mic_requires_model() {
        let mut settings = Settings::default();
        assert!(!wake_would_open_mic(&settings));
        settings.wake_word_enabled = true;
        assert!(!wake_would_open_mic(&settings));
        settings.wake_word_model = Some(std::path::PathBuf::from("wake/stub.onnx"));
        assert!(wake_would_open_mic(&settings));
    }
}
