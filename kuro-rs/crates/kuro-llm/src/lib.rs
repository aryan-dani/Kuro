//! LLM provider port (ADR 0011). Engine wiring lands later.

mod agent;
mod agent_llm;
mod audit;
mod fake;
mod null;
mod planner;
mod provider;
mod tool_plan;
mod turn;

pub use agent::AgentConfig;
pub use agent_llm::AgentLlm;
pub use audit::{AuditEntry, AuditLog};
pub use fake::FakeLlm;
pub use null::NullLlm;
pub use planner::{PlannerLlm, extract_json_array, planner_system_prompt};
pub use provider::LlmProvider;
pub use tool_plan::{ToolPlan, parse_tool_plans_json};
pub use turn::{AgentRunResult, TurnRequest, execute_confirmed_plan, run_agent_request, run_agent_turn};
