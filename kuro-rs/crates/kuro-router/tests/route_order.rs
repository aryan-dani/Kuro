use kuro_router::{route_command, try_handle};
use kuro_tools::n8n::{self, Workflow};

fn fixture() -> Vec<Workflow> {
    vec![Workflow {
        id: "morning_brief".into(),
        webhook: "morning-brief".into(),
        description: "Today".into(),
        aliases: vec!["morning brief".into()],
        mode: "sync".into(),
        takes_input: false,
        spoken_name: "morning brief".into(),
    }]
}

#[test]
fn routes_in_priority_order_section_5_2() {
    n8n::set_loaded(Some(fixture()));
    let exact = [
        ("cancel", "Alright."),
        ("never mind", "Alright."),
        ("thank you", "Anytime."),
        ("who are you", "Kuro. Local assistant. Phrases first, model last."),
        ("stop the music", "Music stopped."),
    ];
    for (phrase, want) in exact {
        assert_eq!(try_handle(phrase).unwrap().reply, want);
    }
    let desk = try_handle("desk off").unwrap().reply;
    assert!(!desk.trim().is_empty());
    let on = try_handle("desk on").unwrap().reply;
    assert!(!on.trim().is_empty());
    let help = try_handle("help").unwrap().reply;
    assert!(help.contains("Desk, music, news"));
    assert!(try_handle("pause the song").is_some());
    assert!(try_handle("resume music").is_some());
    assert!(!try_handle("good night").unwrap().reply.trim().is_empty());
    assert!(!try_handle("what's the news").unwrap().reply.trim().is_empty());
    assert!(!try_handle("check my inbox").unwrap().reply.trim().is_empty());
    let first = try_handle("daddy's home").unwrap().reply;
    assert!(first.contains("Welcome"));
    let second = try_handle("wakeup daddy").unwrap().reply;
    assert!(second.contains("Already on it"));
    let hint = route_command("run n8n automation please", true, None, "always", true);
    assert!(hint.reply.contains("workflow"));
    assert!(!hint.start_ollama);
    n8n::set_loaded(None);
}
