use kuro_tools::{execute_tool, registered_tool_names};
use serde_json::{Value, json};

fn fuzz_args() -> Vec<Value> {
    vec![
        json!({}),
        json!(null),
        json!([]),
        json!({"scene": 99}),
        json!({"topic": true}),
        json!({"workflow": {"nested": "bad"}}),
        json!({"contact": "", "source": "nowhere", "text": 1}),
        json!({"mode": "not-a-mode"}),
    ]
}

#[test]
fn every_tool_survives_malformed_input() {
    for name in registered_tool_names() {
        for args in fuzz_args() {
            let out = execute_tool(name, &args);
            assert!(!out.trim().is_empty(), "{name} returned empty for {args}");
        }
        let bad_name = format!("{name}_nope");
        let unknown = execute_tool(&bad_name, &json!({}));
        assert!(!unknown.trim().is_empty());
    }
}
