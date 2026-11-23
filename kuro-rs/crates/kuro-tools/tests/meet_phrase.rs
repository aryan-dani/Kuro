use kuro_tools::meet::{match_phrase, run, set_signed_in};

#[test]
fn join_phrases_match() {
    for text in ["join the meet", "join that meet link", "join the google meet link you sent to siya"] {
        let found = match_phrase(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(found.get("action").map(String::as_str), Some("join"), "{text}");
    }
    let named = match_phrase("join the google meet link you sent to siya").unwrap();
    assert_eq!(named.get("contact").map(String::as_str), Some("siya"));
    assert!(match_phrase("open the meet you sent to siya").is_none());
}

#[test]
fn send_meet_ignores_leading_chatter() {
    let found = match_phrase("hello kuro now can you actually send a google meet link to sia").unwrap();
    assert_eq!(found.get("action").map(String::as_str), Some("create"));
    assert_eq!(found.get("contact").map(String::as_str), Some("sia"));
}

#[test]
fn make_and_send_still_matches() {
    let found = match_phrase("Make a Google Meet and send the link to Mom").unwrap();
    assert_eq!(found.get("action").map(String::as_str), Some("create"));
    assert_eq!(found.get("contact").map(String::as_str), Some("mom"));
}

#[test]
fn buried_meet_in_long_ramble_stays_unmatched() {
    assert!(match_phrase(
        "Now you should be better off right. Comparatively at least. Let's see if you understand my query. And if you do send a Google Meet link to Mom."
    )
    .is_none());
}

#[test]
fn create_phrase_rejects_chatter_and_meet_named_contacts() {
    assert!(match_phrase("I need to restart after the meet fix").is_none());
    assert!(match_phrase("open the start menu").is_none());
    assert!(match_phrase("Send the Google Meet link to General Meet").is_none());
    assert!(match_phrase("create a google meet link please").is_some());
}

#[test]
fn live_create_blocked_without_engine_enable() {
    set_signed_in(None);
    let reply = run(&serde_json::json!({"contact": ""}));
    assert_eq!(reply, "Google Meet is not signed in.");
}
