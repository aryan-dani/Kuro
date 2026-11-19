use serde::Deserialize;
use serde_json::Value;

/// One LLM-proposed feature invocation (phase 26A).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPlan {
    pub feature_id: String,
    pub slots: Value,
    pub summary: String,
}

#[derive(Deserialize)]
struct RawToolPlan {
    feature_id: String,
    #[serde(default)]
    slots: Value,
    #[serde(default)]
    summary: String,
}

/// Parse a single JSON array line into tool plans (tests and scripted LLM output).
pub fn parse_tool_plans_json(text: &str) -> Vec<ToolPlan> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let raw: Vec<RawToolPlan> = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    raw.into_iter().map(|p| ToolPlan { feature_id: p.feature_id, slots: p.slots, summary: p.summary }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_plan_array() {
        let text = r#"[{"feature_id":"desk","slots":{"scene":"power_on"},"summary":"Desk on."}]"#;
        let plans = parse_tool_plans_json(text);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].feature_id, "desk");
        assert_eq!(plans[0].slots, json!({"scene": "power_on"}));
    }

    #[test]
    fn empty_and_invalid_return_empty() {
        assert!(parse_tool_plans_json("").is_empty());
        assert!(parse_tool_plans_json("not json").is_empty());
    }
}
