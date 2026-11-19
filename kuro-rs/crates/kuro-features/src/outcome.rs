use kuro_core::FollowUp;
use serde_json::Value;

/// User-facing wording. Built from manifest reply keys plus named arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spoken(pub String);

impl Spoken {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub speech: Spoken,
    pub follow_up: FollowUp,
    pub data: Option<Value>,
}

impl Outcome {
    pub fn say(text: impl Into<String>) -> Self {
        Self { speech: Spoken::new(text), follow_up: FollowUp::None, data: None }
    }
}
