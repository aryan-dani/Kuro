use crate::trigger::{Activation, Trigger};

#[derive(Debug)]
pub struct FakeDetector {
    id: String,
    pending: bool,
}

impl FakeDetector {
    pub fn once(id: impl Into<String>) -> Self {
        Self { id: id.into(), pending: true }
    }
}

impl Trigger for FakeDetector {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn poll(&mut self) -> Option<Activation> {
        if self.pending {
            self.pending = false;
            return Some(Activation { source_id: self.id.clone() });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_once_then_silent() {
        let mut detector = FakeDetector::once("fake.wake");
        assert_eq!(detector.poll().map(|a| a.source_id), Some("fake.wake".to_string()));
        assert!(detector.poll().is_none());
    }
}
