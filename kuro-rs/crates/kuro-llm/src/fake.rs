use std::collections::VecDeque;
use std::sync::Mutex;

use crate::agent_llm::AgentLlm;
use crate::provider::LlmProvider;
use crate::tool_plan::ToolPlan;

#[derive(Debug)]
pub struct FakeLlm {
    replies: Mutex<VecDeque<String>>,
    fallback: String,
    plan_batches: Mutex<VecDeque<Vec<ToolPlan>>>,
}

impl FakeLlm {
    pub fn scripted(replies: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let queue: VecDeque<String> = replies.into_iter().map(Into::into).collect();
        Self { replies: Mutex::new(queue), fallback: String::new(), plan_batches: Mutex::new(VecDeque::new()) }
    }

    pub fn with_fallback(replies: impl IntoIterator<Item = impl Into<String>>, fallback: impl Into<String>) -> Self {
        let queue: VecDeque<String> = replies.into_iter().map(Into::into).collect();
        Self { replies: Mutex::new(queue), fallback: fallback.into(), plan_batches: Mutex::new(VecDeque::new()) }
    }

    pub fn scripted_plans(batches: impl IntoIterator<Item = impl IntoIterator<Item = ToolPlan>>) -> Self {
        let queue: VecDeque<Vec<ToolPlan>> = batches.into_iter().map(|b| b.into_iter().collect()).collect();
        Self { replies: Mutex::new(VecDeque::new()), fallback: String::new(), plan_batches: Mutex::new(queue) }
    }
}

impl LlmProvider for FakeLlm {
    fn complete(&self, _prompt: &str) -> String {
        let mut guard = self.replies.lock().expect("fake llm lock");
        guard.pop_front().unwrap_or_else(|| self.fallback.clone())
    }
}

impl AgentLlm for FakeLlm {
    fn plan_tools(&self, _user_text: &str) -> Vec<ToolPlan> {
        let mut guard = self.plan_batches.lock().expect("fake llm plan lock");
        guard.pop_front().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_scripted_replies_in_order() {
        let llm = FakeLlm::scripted(["one", "two"]);
        assert_eq!(llm.complete("a"), "one");
        assert_eq!(llm.complete("b"), "two");
    }

    #[test]
    fn uses_fallback_when_queue_empty() {
        let llm = FakeLlm::with_fallback(std::iter::empty::<String>(), "done");
        assert_eq!(llm.complete("x"), "done");
    }

    #[test]
    fn pops_plan_batches_in_order() {
        let llm = FakeLlm::scripted_plans([
            vec![ToolPlan { feature_id: "a".into(), slots: serde_json::Value::Null, summary: String::new() }],
            vec![],
        ]);
        assert_eq!(llm.plan_tools("x").len(), 1);
        assert!(llm.plan_tools("x").is_empty());
    }
}
