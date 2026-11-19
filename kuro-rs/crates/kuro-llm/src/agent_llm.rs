use crate::provider::LlmProvider;
use crate::tool_plan::{ToolPlan, parse_tool_plans_json};

/// LLM port extension for structured tool plans (phase 26A).
pub trait AgentLlm: LlmProvider {
    fn plan_tools(&self, user_text: &str) -> Vec<ToolPlan>;

    /// Default: try to parse a JSON plan line from `complete`.
    fn plan_tools_via_complete(&self, user_text: &str) -> Vec<ToolPlan> {
        let prompt = format!("plan tools for: {user_text}");
        parse_tool_plans_json(&self.complete(&prompt))
    }
}
