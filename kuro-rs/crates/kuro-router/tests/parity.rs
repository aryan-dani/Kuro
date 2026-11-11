use kuro_router::{is_power_off, is_power_on, is_whatsapp, matches, normalize, try_handle};
use kuro_tools::n8n::{self, Workflow};
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
fn phrases_matches_routes() {
    n8n::set_loaded(Some(fixture()));
    assert!(matches("please run the morning brief"));
    assert!(matches("log idea build the nfc reader"));
    assert!(!matches("run banana radio"));
    assert!(matches("desk off"));
    assert!(matches("what's the news today"));
}

#[test]
fn clipboard_is_not_desk_power() {
    let t = normalize("Send my clipboard to me on WhatsApp.");
    assert!(is_whatsapp(&t));
    assert!(!is_power_on(&t));
    assert!(!is_power_off(&t));
}

#[test]
fn phrases_matches_whatsapp() {
    assert!(matches("send this link to siya on whatsapp"));
    assert!(matches("whatsapp siya the link"));
    assert!(matches("pause the song"));
    assert!(!matches("hello there"));
}

#[test]
fn meet_phrase_routes_without_google() {
    kuro_tools::set_executor(Some(Box::new(|name, args| {
        assert_eq!(name, "make_meet");
        if args.get("contact").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
            "Success: The Meet is open.".into()
        } else {
            assert_eq!(args["contact"], "mom");
            "Success: Sent the Meet link to Mom.".into()
        }
    })));
    let sent = try_handle("Make a Google Meet and send the link to Mom").unwrap();
    assert!(sent.force_fast_tts);
    assert_eq!(sent.reply, "Sent the Meet link to Mom.");
    let opened = try_handle("Make a Google Meet").unwrap();
    assert!(opened.force_fast_tts);
    assert_eq!(opened.reply, "The Meet is open.");
    kuro_tools::set_executor(None);
    let _ = json!({});
}

#[test]
fn meet_samples_match_router() {
    for text in [
        "Make a Google Meet and send the link to Mom",
        "Make a Meet and send it to Mom",
        "Create a Google Meet for Mom",
        "please make a google meet and send the link to mom on whatsapp",
        "Make a Google Meet",
        "start a meet",
        "Send the Google mate link to mob.",
        "Send the Google Meet link to Mom",
    ] {
        assert!(matches(text), "{text}");
    }
}

#[test]
fn prefer_live_phrase() {
    let chosen = kuro_router::prefer_user_text("desk off", "hello there");
    assert_eq!(chosen, "desk off");
}
