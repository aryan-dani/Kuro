use crate::agent_llm::AgentLlm;
use crate::provider::LlmProvider;
use crate::tool_plan::ToolPlan;

#[derive(Debug, Default, Clone, Copy)]
pub struct NullLlm;

impl LlmProvider for NullLlm {
    fn complete(&self, _prompt: &str) -> String {
        String::new()
    }
}

impl AgentLlm for NullLlm {
    fn plan_tools(&self, _user_text: &str) -> Vec<ToolPlan> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::agent_llm::AgentLlm;
    use crate::provider::LlmProvider;

    use super::NullLlm;

    #[test]
    fn complete_is_empty() {
        assert_eq!(NullLlm.complete("hi"), "");
    }

    #[test]
    fn plan_tools_is_empty() {
        assert!(NullLlm.plan_tools("hi").is_empty());
    }
}
