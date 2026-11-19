use crate::trigger::{Activation, Trigger};

pub struct TriggerRegistry {
    entries: Vec<Box<dyn Trigger>>,
}

impl Default for TriggerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TriggerRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn register(&mut self, trigger: Box<dyn Trigger>) {
        self.entries.push(trigger);
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|t| t.id()).collect()
    }

    pub fn poll_all(&mut self) -> Vec<Activation> {
        let mut fired = Vec::new();
        for entry in &mut self.entries {
            if let Some(activation) = entry.poll() {
                fired.push(activation);
            }
        }
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeDetector;
    use crate::null::NullWakeWord;

    #[test]
    fn registry_collects_one_shot_activation() {
        let mut registry = TriggerRegistry::new();
        registry.register(Box::new(NullWakeWord));
        registry.register(Box::new(FakeDetector::once("fake.wake")));
        assert_eq!(registry.ids(), vec!["wake_word.null", "fake.wake"]);
        let batch = registry.poll_all();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].source_id, "fake.wake");
        assert!(registry.poll_all().is_empty());
    }
}
