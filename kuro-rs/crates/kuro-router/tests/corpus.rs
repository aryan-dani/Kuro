//! Golden routing corpus from `kuro-rs/fixtures/phrases/*.toml`.
//! Set `KURO_BLESS=1` to rewrite `normalized` and `reply` fields from live output.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use kuro_router::{normalize, route_semantic, try_handle};
use kuro_tools::n8n::{self, Workflow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
struct CaseFile {
    #[serde(default)]
    case: Vec<Case>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Case {
    id: String,
    input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    normalized: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    args: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reply: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reply_contains: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    handled: Option<bool>,
    #[serde(default)]
    needs_model: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    known_bug: Option<String>,
}

#[derive(Clone)]
struct CallRecord {
    name: String,
    args: Value,
}

fn calls() -> &'static Mutex<Vec<CallRecord>> {
    static CALLS: OnceLock<Mutex<Vec<CallRecord>>> = OnceLock::new();
    CALLS.get_or_init(|| Mutex::new(Vec::new()))
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/phrases")
}

fn workflow_fixture() -> Vec<Workflow> {
    vec![
        Workflow {
            id: "morning_brief".into(),
            webhook: "morning-brief".into(),
            description: "Today".into(),
            aliases: vec!["morning brief".into(), "my morning".into(), "start my day".into()],
            mode: "sync".into(),
            takes_input: false,
            spoken_name: "morning brief".into(),
        },
        Workflow {
            id: "log_idea".into(),
            webhook: "log-idea".into(),
            description: "Notes".into(),
            aliases: vec!["log idea".into(), "save idea".into(), "note this".into()],
            mode: "async".into(),
            takes_input: true,
            spoken_name: "log idea".into(),
        },
        Workflow {
            id: "hackathon_prep".into(),
            webhook: "hackathon-prep".into(),
            description: "Prep".into(),
            aliases: vec!["hackathon prep".into()],
            mode: "sync".into(),
            takes_input: false,
            spoken_name: "hackathon prep".into(),
        },
        Workflow {
            id: "hello".into(),
            webhook: "hello-kuro".into(),
            description: "Ping".into(),
            aliases: vec!["hello workflow".into()],
            mode: "sync".into(),
            takes_input: false,
            spoken_name: "hello workflow".into(),
        },
    ]
}

fn scrub_env() {
    let doomed: Vec<String> = std::env::vars()
        .map(|(k, _)| k)
        .filter(|k| {
            k.starts_with("TUYA_") || k.starts_with("KURO_N8N_") || k.starts_with("KURO_HUB_") || k.starts_with("N8N_")
        })
        .collect();
    for key in doomed {
        unsafe { std::env::remove_var(&key) };
    }
    unsafe {
        std::env::set_var("DADDY_HOME_SONG_PATH", "C:\\nonexistent\\kuro-corpus-missing.mp3");
    }
}

fn install_executor(data_dir: &Path) {
    unsafe {
        std::env::set_var("KURO_DATA_DIR", data_dir);
    }
    calls().lock().unwrap().clear();
    kuro_tools::set_executor(Some(Box::new(|name, args| {
        calls().lock().unwrap().push(CallRecord { name: name.to_string(), args: args.clone() });
        match name {
            "control_smart_plug" | "control_smart_bulb" => "Success: ok".into(),
            "get_news" => "- [1] Alpha: first story\n- [2] Beta: second story\n- [3] Gamma: third story".into(),
            "check_inbox" => "Two unread messages.".into(),
            "make_meet" => {
                if args.get("contact").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
                    "Success: The Meet is open.".into()
                } else {
                    let contact = args["contact"].as_str().unwrap_or("contact");
                    format!("Success: Sent the Meet link to {contact}.")
                }
            }
            "join_meet" => "Success: The Meet is open.".into(),
            "send_whatsapp" => "Success: Sent.".into(),
            "run_workflow" => "Success: Workflow started.".into(),
            "read_screen" => "Success: Screen says hello.".into(),
            _ => format!("Success: {name}"),
        }
    })));
}

fn intent_name(normalized: &str) -> Option<String> {
    match route_semantic(normalized)? {
        kuro_router::RouteDecision::Llm => Some("Llm".into()),
        kuro_router::RouteDecision::Tool(intent) => Some(format!("{intent:?}")),
    }
}

fn load_cases(path: &Path) -> Vec<Case> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let parsed: CaseFile = toml::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    parsed.case
}

fn write_blessed(path: &Path, cases: &[Case]) {
    let mut out = String::new();
    if let Ok(raw) = std::fs::read_to_string(path) {
        for line in raw.lines() {
            if line.starts_with('#') {
                out.push_str(line);
                out.push('\n');
            } else {
                break;
            }
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
    }
    for case in cases {
        out.push_str("[[case]]\n");
        out.push_str(&format!("id = {:?}\n", case.id));
        out.push_str(&format!("input = {:?}\n", case.input));
        if let Some(v) = &case.normalized {
            out.push_str(&format!("normalized = {:?}\n", v));
        }
        if let Some(v) = &case.stage {
            out.push_str(&format!("stage = {:?}\n", v));
        }
        if let Some(v) = &case.intent {
            out.push_str(&format!("intent = {:?}\n", v));
        }
        if let Some(v) = &case.tool {
            out.push_str(&format!("tool = {:?}\n", v));
        }
        if let Some(v) = &case.args {
            out.push_str(&format!("args = {}\n", toml_inline_table(v)));
        }
        if let Some(v) = &case.reply {
            out.push_str(&format!("reply = {:?}\n", v));
        }
        if let Some(v) = &case.reply_contains {
            out.push_str(&format!("reply_contains = {:?}\n", v));
        }
        if let Some(v) = case.handled {
            out.push_str(&format!("handled = {}\n", v));
        }
        if case.needs_model {
            out.push_str("needs_model = true\n");
        }
        if let Some(v) = &case.known_bug {
            out.push_str(&format!("known_bug = {:?}\n", v));
        }
        out.push('\n');
    }
    std::fs::write(path, out).expect("bless write");
}

fn stable_contains_fragment(reply: &str) -> String {
    for needle in ["Welcome", "Already on it", "Alright", "Anytime", "Desk on", "Desk off", "Music", "Meet"] {
        if reply.contains(needle) {
            return needle.to_string();
        }
    }
    reply.chars().take(12).collect()
}

fn toml_inline_table(value: &Value) -> String {
    let Some(obj) = value.as_object() else {
        return value.to_string();
    };
    let mut parts = Vec::new();
    for (k, v) in obj {
        let rendered = match v {
            Value::String(s) => format!("{k} = {s:?}"),
            Value::Bool(b) => format!("{k} = {b}"),
            Value::Number(n) => format!("{k} = {n}"),
            other => format!("{k} = {other}"),
        };
        parts.push(rendered);
    }
    format!("{{ {} }}", parts.join(", "))
}

#[test]
fn golden_phrase_corpus() {
    scrub_env();
    let data_dir = std::env::temp_dir().join(format!("kuro-corpus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data_dir);
    std::fs::create_dir_all(&data_dir).unwrap();
    install_executor(&data_dir);
    n8n::set_loaded(Some(workflow_fixture()));

    let dir = fixtures_dir();
    assert!(dir.is_dir(), "missing fixtures dir {}", dir.display());

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
        .collect();
    // Cooldown-sensitive ritual/meet files first so their "first hit" cases are meaningful.
    files.sort_by_key(|p| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let rank = match name {
            "rituals.toml" => 0,
            "meet.toml" => 1,
            "route_order.toml" => 2,
            _ => 3,
        };
        (rank, name.to_string())
    });
    assert_eq!(files.len(), 12, "expected 12 phrase fixture files, found {}", files.len());

    let bless = std::env::var("KURO_BLESS").ok().as_deref() == Some("1");
    let model_ready = kuro_router::semantic::find_onnx().is_some();
    let mut skipped_model = 0usize;
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for path in &files {
        let mut cases = load_cases(path);
        assert!(cases.len() >= 8, "{} has fewer than 8 cases", path.display());
        for case in &mut cases {
            total += 1;
            if case.needs_model && !model_ready {
                skipped_model += 1;
                continue;
            }
            calls().lock().unwrap().clear();
            let normalized = normalize(&case.input);
            if bless {
                case.normalized = Some(normalized.clone());
            } else if let Some(expected) = &case.normalized
                && expected != &normalized
            {
                failures.push(format!("{}: normalized want {expected:?} got {normalized:?}", case.id));
            }

            let actual_intent = intent_name(&normalized);
            if bless {
                case.intent = actual_intent.clone();
            } else if model_ready && let Some(expected_intent) = &case.intent {
                match &actual_intent {
                    Some(name) if name == expected_intent => {}
                    Some(name) => failures.push(format!("{}: intent want {expected_intent:?} got {name:?}", case.id)),
                    None => failures.push(format!("{}: intent want {expected_intent:?} got None", case.id)),
                }
            }

            let handled = try_handle(&case.input);
            let is_handled = handled.is_some();
            if !bless && !model_ready && case.handled == Some(true) && !is_handled && case.intent.is_some() {
                skipped_model += 1;
                continue;
            }
            if bless {
                case.handled = Some(is_handled);
            } else if let Some(want) = case.handled
                && want != is_handled
            {
                failures.push(format!(
                    "{}: handled want {want} got {is_handled} (reply={:?})",
                    case.id,
                    handled.as_ref().map(|h| h.reply.clone())
                ));
            }

            if let Some(result) = &handled {
                if bless {
                    if result.reply.starts_with("It's ") || result.reply.starts_with("Today is ") {
                        case.reply = None;
                        if case.reply_contains.is_none() {
                            case.reply_contains =
                                Some(if result.reply.starts_with("It's ") { "It's".into() } else { "Today is".into() });
                        }
                    } else {
                        case.reply = Some(result.reply.clone());
                    }
                } else if let Some(want) = &case.reply
                    && &result.reply != want
                {
                    failures.push(format!("{}: reply want {want:?} got {:?}", case.id, result.reply));
                }
                if let Some(needle) = &case.reply_contains
                    && !result.reply.contains(needle)
                {
                    if bless {
                        case.reply_contains = Some(stable_contains_fragment(&result.reply));
                    } else {
                        failures.push(format!("{}: reply_contains {needle:?} missing in {:?}", case.id, result.reply));
                    }
                }
            } else if bless {
                case.reply = None;
                case.reply_contains = None;
            }

            let recorded = calls().lock().unwrap().clone();
            if bless {
                if let Some(call) = recorded.last() {
                    case.tool = Some(call.name.clone());
                    case.args = Some(call.args.clone());
                } else {
                    case.tool = None;
                    case.args = None;
                }
            } else if let Some(tool) = &case.tool {
                match recorded.last() {
                    Some(call) if &call.name == tool => {
                        if let Some(want_args) = &case.args
                            && &call.args != want_args
                        {
                            failures.push(format!("{}: args want {want_args} got {}", case.id, call.args));
                        }
                    }
                    Some(call) => failures.push(format!("{}: tool want {tool:?} got {:?}", case.id, call.name)),
                    None => failures.push(format!("{}: tool want {tool:?} got no call", case.id)),
                }
            }
        }
        if bless {
            write_blessed(path, &cases);
        }
    }

    kuro_tools::set_executor(None);
    n8n::set_loaded(None);
    let _ = std::fs::remove_dir_all(&data_dir);

    eprintln!("corpus cases={total} skipped_model={skipped_model} failures={}", failures.len());
    assert!(total >= 250, "need at least 250 cases, got {total}");
    if !failures.is_empty() {
        let preview: Vec<_> = failures.iter().take(40).cloned().collect();
        panic!("{} corpus failures (showing up to 40):\n{}", failures.len(), preview.join("\n"));
    }
}

#[test]
fn corpus_env_scrub_blocks_remote_keys() {
    scrub_env();
    let leaked: HashMap<_, _> = std::env::vars()
        .filter(|(k, _)| {
            k.starts_with("TUYA_") || k.starts_with("KURO_N8N_") || k.starts_with("KURO_HUB_") || k.starts_with("N8N_")
        })
        .collect();
    assert!(leaked.is_empty(), "env scrub left keys: {leaked:?}");
}
