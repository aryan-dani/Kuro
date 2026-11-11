use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use serde_json::{Value, json};

pub mod brain;
pub mod feeds;
pub mod hub;
pub mod meet;
pub mod music;
pub mod n8n;
pub mod plug;
pub mod screen;
pub mod tuya;
pub mod whatsapp;

type ToolHook = Box<dyn Fn(&str, &Value) -> String>;

thread_local! {
    static HOOK: RefCell<Option<ToolHook>> = RefCell::new(None);
}

pub fn set_executor(hook: Option<ToolHook>) {
    HOOK.with(|slot| *slot.borrow_mut() = hook);
}

/// One tool entry. Add a new tool with one type plus one `default_registry` line.
#[derive(Clone)]
pub struct KuroTool {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters_schema: Value,
    pub is_local_only: bool,
    pub match_phrase: Option<fn(&str) -> Option<Value>>,
    pub execute: fn(&Value) -> String,
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: Vec<Arc<KuroTool>>,
}

impl ToolRegistry {
    pub fn default_registry() -> Self {
        Self {
            tools: vec![
                Arc::new(KuroTool {
                    name: "control_smart_plug",
                    description: "Control the desk smart plug scenes.",
                    parameters_schema: json!({"type":"object","properties":{"scene":{"type":"string"}}}),
                    is_local_only: true,
                    match_phrase: None,
                    execute: plug::run,
                }),
                Arc::new(KuroTool {
                    name: "control_smart_bulb",
                    description: "Control the desk smart bulb.",
                    parameters_schema: json!({"type":"object","properties":{"action":{"type":"string"}}}),
                    is_local_only: true,
                    match_phrase: None,
                    execute: plug::bulb_run,
                }),
                Arc::new(KuroTool {
                    name: "get_news",
                    description: "Fetch short news headlines.",
                    parameters_schema: json!({"type":"object","properties":{"topic":{"type":"string"}}}),
                    is_local_only: false,
                    match_phrase: None,
                    execute: feeds::run,
                }),
                Arc::new(KuroTool {
                    name: "check_inbox",
                    description: "Summarize urgent mail or deadlines.",
                    parameters_schema: json!({"type":"object","properties":{"mode":{"type":"string"}}}),
                    is_local_only: false,
                    match_phrase: None,
                    execute: feeds::inbox_run,
                }),
                Arc::new(KuroTool {
                    name: "read_screen",
                    description: "Read on-screen text.",
                    parameters_schema: json!({"type":"object"}),
                    is_local_only: true,
                    match_phrase: None,
                    execute: feeds::screen_run,
                }),
                Arc::new(KuroTool {
                    name: "make_meet",
                    description: "Open or share a Google Meet link.",
                    parameters_schema: json!({"type":"object","properties":{"contact":{"type":"string"}}}),
                    is_local_only: false,
                    match_phrase: Some(meet_match),
                    execute: meet::run,
                }),
                Arc::new(KuroTool {
                    name: "send_whatsapp",
                    description: "Send a WhatsApp message to a contact.",
                    parameters_schema: json!({"type":"object","properties":{"contact":{"type":"string"},"source":{"type":"string"},"text":{"type":"string"}}}),
                    is_local_only: false,
                    match_phrase: Some(whatsapp_match),
                    execute: whatsapp::run,
                }),
                Arc::new(KuroTool {
                    name: "run_workflow",
                    description: "Trigger an n8n workflow by phrase or id.",
                    parameters_schema: json!({"type":"object","properties":{"workflow":{"type":"string"},"input":{"type":"string"}}}),
                    is_local_only: false,
                    match_phrase: Some(workflow_match),
                    execute: n8n::run,
                }),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<KuroTool>> {
        self.tools.iter().find(|tool| tool.name == name).cloned()
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|tool| tool.name).collect()
    }

    pub fn match_phrase(&self, text: &str) -> Option<(Arc<KuroTool>, Value)> {
        for tool in &self.tools {
            if let Some(matcher) = tool.match_phrase
                && let Some(args) = matcher(text)
            {
                return Some((tool.clone(), args));
            }
        }
        None
    }

    pub fn execute(&self, name: &str, arguments: &Value) -> String {
        let Some(tool) = self.get(name) else {
            return format!("Error: Tool '{name}' is not recognized.");
        };
        match catch_unwind(AssertUnwindSafe(|| (tool.execute)(arguments))) {
            Ok(text) => {
                if text.trim().is_empty() {
                    "Done, sir.".into()
                } else {
                    text
                }
            }
            Err(_) => "Something went wrong with that tool.".into(),
        }
    }
}

pub fn registered_tool_names() -> Vec<&'static str> {
    ToolRegistry::default_registry().names()
}

pub fn execute_tool(name: &str, arguments: &Value) -> String {
    if let Some(result) = HOOK.with(|slot| slot.borrow().as_ref().map(|f| f(name, arguments))) {
        return result;
    }
    ToolRegistry::default_registry().execute(name, arguments)
}

pub fn match_registered_phrase(text: &str) -> Option<(Arc<KuroTool>, Value)> {
    ToolRegistry::default_registry().match_phrase(text)
}

fn meet_match(text: &str) -> Option<Value> {
    let found = meet::match_phrase(text)?;
    Some(json!({
        "contact": found.get("contact").cloned().unwrap_or_default(),
    }))
}

fn whatsapp_match(text: &str) -> Option<Value> {
    let found = whatsapp::match_phrase(text)?;
    Some(json!({
        "contact": found.get("contact").cloned().unwrap_or_default(),
        "source": found.get("source").cloned().unwrap_or_default(),
        "text": "",
    }))
}

fn workflow_match(text: &str) -> Option<Value> {
    let rows = n8n::load_workflows();
    let (wf, extra) = n8n::match_phrase(text, &rows)?;
    Some(json!({
        "workflow": wf.id,
        "input": extra,
    }))
}
