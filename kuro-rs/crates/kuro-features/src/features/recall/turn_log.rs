//! In-memory turn log for the recall first slice (Store lands in phase 20).

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TurnLog {
    last_spoken: Option<String>,
    last_question: Option<String>,
}

impl TurnLog {
    pub fn record_spoken(&mut self, text: impl Into<String>) {
        self.last_spoken = Some(text.into());
    }

    pub fn record_question(&mut self, text: impl Into<String>) {
        self.last_question = Some(text.into());
    }

    pub fn last_spoken(&self) -> Option<&str> {
        self.last_spoken.as_deref()
    }

    pub fn last_question(&self) -> Option<&str> {
        self.last_question.as_deref()
    }
}
