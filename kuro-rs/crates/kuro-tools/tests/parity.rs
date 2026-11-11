use std::collections::HashMap;

use kuro_tools::brain::Brain;
use kuro_tools::n8n::{self, OpenError, Workflow};
use kuro_tools::whatsapp;
use serde_json::json;

fn fixture() -> Vec<Workflow> {
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
    ]
}

#[test]
fn alias_exact_and_fuzzy() {
    let rows = fixture();
    assert_eq!(n8n::resolve_workflow("morning brief", &rows).unwrap().id, "morning_brief");
    assert_eq!(n8n::resolve_workflow("morning_brief", &rows).unwrap().id, "morning_brief");
    assert_eq!(n8n::resolve_workflow("morning breif", &rows).unwrap().id, "morning_brief");
    assert!(n8n::resolve_workflow("banana radio", &rows).is_none());
    assert!(n8n::resolve_workflow("zzzzzzzzzzzz", &rows).is_none());
}

#[test]
fn input_capture() {
    let rows = fixture();
    let (wf, extra) = n8n::match_phrase("log idea build the nfc reader", &rows).unwrap();
    assert_eq!(wf.id, "log_idea");
    assert_eq!(extra, "build the nfc reader");
    let (_wf, extra) = n8n::match_phrase("note this ship the reader", &rows).unwrap();
    assert_eq!(extra, "ship the reader");
}

#[test]
fn phrase_phrasings() {
    let rows = fixture();
    let samples = [
        "run morning brief",
        "please run the morning brief",
        "hey kuro start my morning",
        "can you launch hackathon prep",
        "trigger morning brief",
        "start my day",
        "morning brief",
        "my morning",
        "please log idea buy milk",
        "save idea the drawer sensor",
        "kindly run hackathon prep",
        "n8n is not this",
    ];
    let hits: Vec<_> = samples.iter().map(|text| n8n::match_phrase(text, &rows)).collect();
    assert_eq!(hits[0].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[1].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[2].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[3].as_ref().unwrap().0.id, "hackathon_prep");
    assert_eq!(hits[4].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[5].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[5].as_ref().unwrap().1, "");
    assert_eq!(hits[6].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[7].as_ref().unwrap().0.id, "morning_brief");
    assert_eq!(hits[8].as_ref().unwrap().0.id, "log_idea");
    assert_eq!(hits[8].as_ref().unwrap().1, "buy milk");
    assert!(hits[9].as_ref().unwrap().1.contains("drawer"));
    assert_eq!(hits[10].as_ref().unwrap().0.id, "hackathon_prep");
    assert!(hits[11].is_none());
}

fn wf(mode: &str) -> Workflow {
    Workflow {
        id: "hello_kuro".into(),
        webhook: "hello-kuro".into(),
        description: String::new(),
        aliases: vec![],
        mode: mode.into(),
        takes_input: false,
        spoken_name: "hello kuro".into(),
    }
}

#[test]
fn http_cases() {
    n8n::set_config(Some("http://100.78.60.50:5678/webhook".into()), Some("test-token".into()));
    let sync = wf("sync");
    assert!(
        n8n::dispatch(&sync, "", Some(&|_url, _t| Ok((200, "{\"reply\":\"Hello from n8n\"}".into()))))
            .ends_with("Hello from n8n")
    );
    assert_eq!(n8n::dispatch(&sync, "", Some(&|_url, _t| Ok((200, "{}".into())))), "Success: Done, sir.");
    assert_eq!(n8n::dispatch(&sync, "", Some(&|_url, _t| Err(OpenError::Http(401)))), "n8n rejected my token.");
    assert_eq!(
        n8n::dispatch(&sync, "", Some(&|_url, _t| Err(OpenError::Http(404)))),
        "That workflow is not active in n8n."
    );
    assert_eq!(n8n::dispatch(&sync, "", Some(&|_url, _t| Err(OpenError::Timeout))), "n8n timed out.");
    assert_eq!(n8n::dispatch(&sync, "", Some(&|_url, _t| Err(OpenError::Unreachable))), "I can't reach n8n.");
    assert_eq!(
        n8n::dispatch(&sync, "", Some(&|_url, _t| Ok((200, "not json".into())))),
        "n8n returned something I could not read."
    );
    assert_eq!(
        n8n::dispatch(&wf("async"), "note", Some(&|_url, _t| Ok((200, String::new())))),
        "Success: Started hello kuro."
    );
}

#[test]
fn unconfigured() {
    n8n::set_config(Some(String::new()), Some(String::new()));
    assert_eq!(n8n::dispatch(&wf("sync"), "", Some(&|_url, _t| panic!("called"))), "n8n is not configured.");
}

#[test]
fn registry_parse() {
    let dir = std::env::temp_dir().join(format!("kuro-n8n-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("n8n_workflows.json");
    std::fs::write(&path, serde_json::json!({"workflows": {"hello_kuro": {"webhook": "hello-kuro", "description": "Hi", "aliases": ["Hello Workflow!"], "mode": "nope", "takes_input": false}}}).to_string()).unwrap();
    n8n::set_loaded(None);
    n8n::set_registry_path(Some(path));
    let found = n8n::load_workflows();
    assert_eq!(found[0].aliases, vec!["hello workflow".to_string()]);
    assert_eq!(found[0].mode, "sync");
    n8n::set_registry_path(Some(dir.join("missing.json")));
    assert!(n8n::load_workflows().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn follow_up_keeps_earlier_turns() {
    let mut brain = Brain::default();
    brain.thread("one");
    brain.remember("ack");
    let messages = brain.thread("two");
    let contents: Vec<String> =
        messages.iter().map(|m| m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string()).collect();
    assert!(contents.iter().any(|c| c == "one"));
    assert!(contents.iter().any(|c| c == "ack"));
    assert_eq!(contents.last().unwrap(), "two");
}

#[test]
fn thread_caps_at_twenty_turns_and_clear_drops_it() {
    let mut brain = Brain::default();
    for index in 0..25 {
        brain.thread(&format!("u{index}"));
        brain.remember(&format!("a{index}"));
    }
    let rest = &brain.messages[1..];
    assert!(rest.len() <= 40);
    assert_eq!(rest[rest.len() - 2]["content"], "u24");
    assert_eq!(rest[rest.len() - 1]["content"], "a24");
    assert!(rest.iter().all(|item| item["content"] != "u0"));
    brain.clear();
    assert!(brain.messages.is_empty());
}

#[test]
fn whatsapp_helpers_and_phrases() {
    assert_eq!(whatsapp::spoken_recipient("me"), "you");
    assert_eq!(whatsapp::spoken_recipient("Me"), "you");
    assert_eq!(whatsapp::spoken_recipient("siya"), "Siya");
    assert!(whatsapp::target_is_front(10, 10));
    assert!(!whatsapp::target_is_front(10, 11));
    assert!(!whatsapp::target_is_front(0, 10));
    assert!(whatsapp::is_whatsapp_title("WhatsApp"));
    assert!(whatsapp::is_whatsapp_title("(2) WhatsApp"));
    assert!(!whatsapp::is_whatsapp_title("WhatsApp Web - Google Chrome"));
    assert!(!whatsapp::is_whatsapp_title(""));
    let mut formats = std::collections::HashSet::new();
    assert_eq!(whatsapp::payload_kind("hello", &formats), "text");
    formats.insert(2);
    formats.insert(8);
    assert_eq!(whatsapp::payload_kind("", &formats), "image");
    formats.clear();
    formats.insert(13);
    assert_eq!(whatsapp::payload_kind("", &formats), "empty");
    assert_eq!(whatsapp::payload_kind("  ", &std::collections::HashSet::new()), "empty");
    assert_eq!(whatsapp::clean_digits("+91 00000-00000"), "910000000000");
    assert_eq!(whatsapp::clean_digits("no digits"), "");
    assert!(whatsapp::url_ok("https://meet.google.com/abc-defg-hij"));
    assert!(whatsapp::url_ok("http://example.com/a"));
    assert!(!whatsapp::url_ok("not a url"));
    assert!(!whatsapp::url_ok("ftp://example.com"));
    assert!(!whatsapp::url_ok(""));

    let book = HashMap::from([
        ("Mom".into(), "910000000009".into()),
        ("Bob".into(), "910000000008".into()),
        ("Siya".into(), "910000000001".into()),
    ]);
    assert_eq!(whatsapp::resolve_contact("mob", &book).unwrap().0, "Mom");
    assert_eq!(whatsapp::resolve_contact("mom", &book).unwrap().0, "Mom");
    assert!(whatsapp::resolve_contact("zzzznotaperson", &book).is_none());
    let tied = HashMap::from([("Abx".into(), "910000000003".into()), ("Aby".into(), "910000000004".into())]);
    assert!(whatsapp::resolve_contact("abc", &tied).is_none());
    assert_eq!(
        whatsapp::resolve_contact("abc", &HashMap::from([("Abx".into(), "910000000003".into())])).unwrap().0,
        "Abx"
    );

    let book = HashMap::from([("siya".into(), "910000000001".into()), ("aryan".into(), "910000000002".into())]);
    assert_eq!(whatsapp::resolve_contact("siya", &book).unwrap().0, "siya");
    assert_eq!(whatsapp::resolve_contact("Siya", &book).unwrap().0, "siya");
    assert_eq!(whatsapp::resolve_contact("siyaa", &book).unwrap().0, "siya");
    assert!(whatsapp::resolve_contact("somebody else", &book).is_none());
    assert!(whatsapp::resolve_contact("zzzzzzzz", &book).is_none());

    let samples = [
        ("send this link to siya on whatsapp", "browser_url", "siya"),
        ("send the meeting link to siya on whatsapp", "browser_url", "siya"),
        ("send my clipboard to siya via whatsapp", "clipboard", "siya"),
        ("please send the link to siya in whatsapp", "browser_url", "siya"),
        ("whatsapp siya this link", "browser_url", "siya"),
        ("whatsapp siya the link", "browser_url", "siya"),
        ("hey kuro send this link to siya on whatsapp", "browser_url", "siya"),
        ("can you send clipboard to aryan on whatsapp", "clipboard", "aryan"),
        ("send the message to siya on whatsapp", "clipboard", "siya"),
        ("send me my clipboard on whatsapp", "clipboard", "me"),
        ("send me the link on whatsapp", "browser_url", "me"),
        ("send my clipboard on whatsapp", "clipboard", "me"),
        ("send the link on whatsapp", "browser_url", "me"),
        ("send clipboard on whatsapp", "clipboard", "me"),
        ("send my clipboard history on whatsapp", "clipboard", "me"),
        ("send me my clipboard history on whatsapp", "clipboard", "me"),
    ];
    for (text, source, name) in samples {
        let found = whatsapp::match_phrase(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(found.get("source").map(String::as_str), Some(source), "{text}");
        assert_eq!(found.get("contact").map(String::as_str), Some(name), "{text}");
    }
    assert!(whatsapp::match_phrase("desk off").is_none());
    assert!(whatsapp::match_phrase("send this link to siya").is_none());
}

#[test]
fn whatsapp_address_book() {
    let dir = std::env::temp_dir().join(format!("kuro-wa-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db = dir.join("wa.db");
    let con = rusqlite::Connection::open(&db).unwrap();
    con.execute("CREATE TABLE whatsmeow_contacts (our_jid TEXT, their_jid TEXT, first_name TEXT, full_name TEXT, push_name TEXT, business_name TEXT, redacted_phone TEXT)", []).unwrap();
    con.execute(
        "INSERT INTO whatsmeow_contacts VALUES (?,?,?,?,?,?,?)",
        rusqlite::params!["self@s.whatsapp.net", "910000000009@s.whatsapp.net", "", "Mom", "Mom", "", ""],
    )
    .unwrap();
    con.execute(
        "INSERT INTO whatsmeow_contacts VALUES (?,?,?,?,?,?,?)",
        rusqlite::params!["self@s.whatsapp.net", "910000000008@s.whatsapp.net", "", "Sam", "", "", ""],
    )
    .unwrap();
    con.execute(
        "INSERT INTO whatsmeow_contacts VALUES (?,?,?,?,?,?,?)",
        rusqlite::params!["self@s.whatsapp.net", "910000000007@s.whatsapp.net", "", "Sam", "", "", ""],
    )
    .unwrap();
    drop(con);
    whatsapp::set_session_path(Some(db));
    whatsapp::set_contacts(Some(HashMap::from([("me".into(), "910000000001".into())])));
    whatsapp::set_address_book(None);
    let book = whatsapp::address_book();
    let found = whatsapp::resolve_contact("mom", &book).unwrap();
    assert_eq!(found.0, "Mom");
    assert_eq!(found.1, "910000000009");
    assert_eq!(whatsapp::resolve_contact("me", &book).unwrap().0, "me");
    assert!(whatsapp::resolve_contact("sam", &book).is_none());
    assert!(whatsapp::resolve_contact("zzzznotaperson", &book).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn meet_phrases_and_guards() {
    let args = kuro_tools::meet::edge_args(r"C:\Edge\msedge.exe", "https://meet.google.com/abc-defg-hij");
    assert!(args[0].ends_with("msedge.exe"));
    assert_eq!(args[1], "--app=https://meet.google.com/abc-defg-hij");
    assert!(args.iter().any(|a| a == "--start-maximized"));
    assert!(!args.iter().any(|a| a == "--new-window"));
    let samples = [
        ("Make a Google Meet and send the link to Mom", "mom"),
        ("Make a Meet and send it to Mom", "mom"),
        ("Create a Google Meet for Mom", "mom"),
        ("please make a google meet and send the link to mom on whatsapp", "mom"),
        ("Make a Google Meet", ""),
        ("start a meet", ""),
        ("Send the Google mate link to mob.", "mob"),
        ("Send the Google Meet link to Mom", "mom"),
    ];
    for (text, name) in samples {
        let found = kuro_tools::meet::match_phrase(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(found.get("contact").map(String::as_str), Some(name), "{text}");
    }
    assert!(kuro_tools::meet::match_phrase("desk off").is_none());
    assert!(kuro_tools::meet::match_phrase("send the link to mom on whatsapp").is_none());
    assert!(kuro_tools::meet::match_phrase(
        "Now you should be better off right. Comparatively at least. Let's see if you understand my query. And if you do send a Google Meet link to Mom."
    )
    .is_none());
    assert!(kuro_tools::meet::match_phrase("send the meeting notes to mom").is_none());

    whatsapp::set_address_book(Some(HashMap::from([("siya".into(), "910000000001".into())])));
    kuro_tools::meet::set_signed_in(Some(true));
    assert_eq!(kuro_tools::meet::run(&json!({"contact": "zzzznotaperson"})), "I don't know that contact.");
    kuro_tools::meet::set_signed_in(Some(false));
    assert_eq!(kuro_tools::meet::run(&json!({"contact": ""})), "Google Meet is not signed in.");
}

#[test]
fn hub_dedup_and_ack() {
    kuro_tools::hub::reset_seen();
    let first = kuro_tools::hub::handle_event(&json!({"id": "e1", "type": "speak", "payload": {"text": "hi"}}));
    assert_eq!(first.speak.as_deref(), Some("hi"));
    assert_eq!(first.ack["type"], "action_result");
    let second = kuro_tools::hub::handle_event(&json!({"id": "e1", "type": "speak", "payload": {"text": "hi"}}));
    assert!(second.duplicate);
    assert_eq!(kuro_tools::hub::COMMAND_TIMEOUT.as_secs(), 8);
    assert_eq!(kuro_tools::hub::TOOLS_TIMEOUT.as_secs(), 5);
    assert_eq!(kuro_tools::hub::EXEC_TIMEOUT.as_secs(), 30);
    assert_eq!(kuro_tools::hub::HEALTH_TIMEOUT.as_secs(), 3);
    assert_eq!(kuro_tools::hub::ws_url("http://100.78.60.50:18200"), "ws://100.78.60.50:18200/v1/events");
}

#[test]
fn plug_cache_roundtrip() {
    let dir = std::env::temp_dir().join(format!("kuro-plug-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("tuya_endpoint.json");
    kuro_tools::plug::save_cache(&path, "10.0.0.8", 3.4);
    let cached = kuro_tools::plug::load_cache(&path);
    assert_eq!(cached["ip"], "10.0.0.8");
    let _ = std::fs::remove_dir_all(&dir);
}
