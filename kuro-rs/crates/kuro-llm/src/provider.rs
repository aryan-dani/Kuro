/// Non-streaming completion port. Streaming chat lands in a later slice.
pub trait LlmProvider: Send + Sync {
    fn complete(&self, prompt: &str) -> String;
}
