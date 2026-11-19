//! Release-profile panic containment (ADR 0007).
#![allow(clippy::disallowed_methods)]

use kuro_tools::{KuroTool, ToolRegistry};

#[test]
fn tool_panic_returns_error_and_second_call_works() {
    static ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn panic_once(_: &serde_json::Value) -> String {
        if !ONCE.swap(true, std::sync::atomic::Ordering::SeqCst) {
            panic!("tool panic");
        }
        "second ok".into()
    }
    let reg = ToolRegistry::single(KuroTool {
        name: "panic_probe",
        description: "test",
        parameters_schema: serde_json::json!({}),
        is_local_only: true,
        match_phrase: None,
        execute: panic_once,
    });
    let args = serde_json::json!({});
    let first = reg.execute("panic_probe", &args);
    assert_eq!(first, "Something went wrong with that tool.");
    let second = reg.execute("panic_probe", &args);
    assert_eq!(second, "second ok");
}
