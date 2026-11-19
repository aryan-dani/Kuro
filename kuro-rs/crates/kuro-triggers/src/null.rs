use crate::trigger::{Activation, Trigger};

#[derive(Debug, Default)]
pub struct NullWakeWord;

impl Trigger for NullWakeWord {
    fn id(&self) -> &str {
        "wake_word.null"
    }

    fn poll(&mut self) -> Option<Activation> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trigger::Trigger;

    #[test]
    fn disabled_opens_no_stream() {
        let mut null = NullWakeWord;
        assert!(null.poll().is_none());
    }
}
