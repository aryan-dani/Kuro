//! Prompted JSON tool planner for `AgentLlm` (works with Ollama in engine, fakes in tests).

use crate::agent_llm::AgentLlm;
use crate::provider::LlmProvider;
use crate::tool_plan::{ToolPlan, parse_tool_plans_json};

/// Build the system prompt that instructs the model to emit a JSON array of tool plans.
pub fn planner_system_prompt(catalog: &str, contacts: &[String]) -> String {
    let contact_line =
        if contacts.is_empty() { String::new() } else { format!("\nKnown contacts: {}.\n", contacts.join(", ")) };
    format!(
        "You are Kuro's tool planner. Decide if the user's request should call one or more tools.\n\
         Respond with ONLY a JSON array, no prose and no markdown.\n\
         If no tool fits, respond with [].\n\
         Each item must be {{\"feature_id\": \"<id>\", \"slots\": {{...}}, \"summary\": \"<short spoken description>\"}}.\n\
         For whatsapp messages set source to \"text\" (never a name) and put the message in text, rewritten naturally in second person (e.g. \"ask Siya if she's okay\" -> text \"Are you okay?\").\n\
         {contact_line}\nAvailable tools:\n{catalog}"
    )
}

/// Strip optional markdown fences and extract the JSON array slice if present.
pub fn extract_json_array(raw: &str) -> &str {
    let trimmed = raw.trim();
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .map(str::trim)
        .and_then(|s| s.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if let (Some(start), Some(end)) = (unfenced.find('['), unfenced.rfind(']'))
        && start <= end
    {
        return &unfenced[start..=end];
    }
    unfenced
}

/// `AgentLlm` backed by a caller-supplied `(system, user) -> Result<String, String>` completer.
pub struct PlannerLlm<F> {
    system_prompt: String,
    complete_fn: F,
}

impl<F> PlannerLlm<F>
where
    F: Fn(&str, &str) -> Result<String, String> + Send + Sync,
{
    pub fn new(catalog: &str, contacts: &[String], complete_fn: F) -> Self {
        Self { system_prompt: planner_system_prompt(catalog, contacts), complete_fn }
    }
}

impl<F> LlmProvider for PlannerLlm<F>
where
    F: Fn(&str, &str) -> Result<String, String> + Send + Sync,
{
    fn complete(&self, prompt: &str) -> String {
        (self.complete_fn)(&self.system_prompt, prompt).unwrap_or_default()
    }
}

impl<F> AgentLlm for PlannerLlm<F>
where
    F: Fn(&str, &str) -> Result<String, String> + Send + Sync,
{
    fn plan_tools(&self, user_text: &str) -> Vec<ToolPlan> {
        let raw = self.complete(user_text);
        parse_tool_plans_json(extract_json_array(&raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scripted_whatsapp_plan_parses_cleanly() {
        let llm = PlannerLlm::new("- whatsapp: send a WhatsApp message", &["Siya".into()], |sys, user| {
            assert!(sys.contains("Known contacts: Siya"));
            assert!(user.contains("Siya"));
            Ok(r#"```json
[{"feature_id":"whatsapp","slots":{"contact":"Siya","source":"text","text":"Are you okay?"},"summary":"Message Siya on WhatsApp: Are you okay?"}]
```"#
                .into())
        });
        let plans = llm.plan_tools("Kuro, can you please ask Siya on WhatsApp if she's okay?");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].feature_id, "whatsapp");
        assert_eq!(plans[0].slots, json!({"contact":"Siya","source":"text","text":"Are you okay?"}));
        assert_eq!(plans[0].summary, "Message Siya on WhatsApp: Are you okay?");
    }

    #[test]
    fn empty_array_and_garbage_return_empty_plans() {
        let empty = PlannerLlm::new("", &[], |_, _| Ok("[]".into()));
        assert!(empty.plan_tools("tell me a joke").is_empty());

        let garbage = PlannerLlm::new("", &[], |_, _| Ok("Sorry, I cannot help with that.".into()));
        assert!(garbage.plan_tools("ask Siya on WhatsApp if she's okay").is_empty());
    }
}
