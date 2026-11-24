#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptSpec {
    pub id: String,
    pub session: u8,
    pub prompt: String,
    pub expected_text: String,
    pub expected_intent: String,
    pub kind: String,
    pub takes: u8,
}

pub fn conditions_for_session(session: u8) -> Vec<String> {
    match session {
        1 => vec!["quiet".into()],
        2 => vec!["fan".into(), "music".into()],
        _ => vec!["soft".into(), "fast".into(), "far".into(), "tired".into()],
    }
}

#[derive(Clone, Copy)]
struct Seed {
    family: &'static str,
    text: &'static str,
    intent: &'static str,
}

pub fn catalog_for_session(session: u8, contacts: &[String], aliases: &[String]) -> Vec<PromptSpec> {
    let mut out = Vec::new();
    push_commands(&mut out, session);
    push_tools(&mut out, session, contacts, aliases);
    push_negatives(&mut out, session);
    push_specials(&mut out, session);
    out
}

fn push_commands(out: &mut Vec<PromptSpec>, session: u8) {
    const ROWS: &[Seed] = &[
        Seed { family: "desk-on", text: "Desk on", intent: "power_on" },
        Seed { family: "desk-on", text: "Turn the desk on", intent: "power_on" },
        Seed { family: "desk-off", text: "Desk off", intent: "power_off" },
        Seed { family: "desk-off", text: "Turn the desk off", intent: "power_off" },
        Seed { family: "lights-on", text: "Lights on", intent: "power_on" },
        Seed { family: "lights-on", text: "Turn on the lights", intent: "power_on" },
        Seed { family: "lights-off", text: "Lights off", intent: "power_off" },
        Seed { family: "lights-off", text: "Turn off the lights", intent: "power_off" },
        Seed { family: "monitor-on", text: "Monitor on", intent: "power_on" },
        Seed { family: "monitor-on", text: "Turn the monitor on", intent: "power_on" },
        Seed { family: "monitor-off", text: "Monitor off", intent: "power_off" },
        Seed { family: "monitor-off", text: "Turn the monitor off", intent: "power_off" },
        Seed { family: "power-on", text: "Power on", intent: "power_on" },
        Seed { family: "power-on", text: "Turn the power on", intent: "power_on" },
        Seed { family: "power-off", text: "Power off", intent: "power_off" },
        Seed { family: "power-off", text: "Turn the power off", intent: "power_off" },
        Seed { family: "night", text: "Good night", intent: "goodnight" },
        Seed { family: "night", text: "Good night sir", intent: "goodnight" },
        Seed { family: "wake", text: "Daddy's home", intent: "daddys_home" },
        Seed { family: "wake", text: "Wake up daddy's home", intent: "daddys_home" },
        Seed { family: "pause", text: "Pause the song", intent: "pause" },
        Seed { family: "pause", text: "Pause the music", intent: "pause" },
        Seed { family: "resume", text: "Resume the song", intent: "resume" },
        Seed { family: "resume", text: "Resume the music", intent: "resume" },
        Seed { family: "stop-music", text: "Stop the music", intent: "stop_music" },
        Seed { family: "stop-music", text: "Stop the song", intent: "stop_music" },
        Seed { family: "news", text: "What's the news", intent: "news" },
        Seed { family: "news", text: "Tech news", intent: "news" },
        Seed { family: "news", text: "World news", intent: "news" },
        Seed { family: "inbox", text: "Inbox check", intent: "inbox" },
        Seed { family: "inbox", text: "Any urgent mail", intent: "inbox" },
        Seed { family: "inbox", text: "Deadlines today", intent: "inbox" },
        Seed { family: "time", text: "What time is it", intent: "time" },
        Seed { family: "time", text: "Tell me the time", intent: "time" },
        Seed { family: "date", text: "What's the date", intent: "date" },
        Seed { family: "date", text: "What day is it", intent: "date" },
        Seed { family: "cancel", text: "Cancel", intent: "cancel" },
        Seed { family: "cancel", text: "Never mind", intent: "cancel" },
        Seed { family: "thanks", text: "Thank you", intent: "thanks" },
        Seed { family: "thanks", text: "Thanks", intent: "thanks" },
        Seed { family: "help", text: "What can you do", intent: "help" },
        Seed { family: "help", text: "Help", intent: "help" },
        Seed { family: "who", text: "Who are you", intent: "identity" },
        Seed { family: "who", text: "What are you", intent: "identity" },
    ];
    let mut rows: Vec<Seed> = ROWS.to_vec();
    for row in crate::prompts_more::extra_seeds() {
        rows.push(Seed { family: row.family, text: row.text, intent: row.intent });
    }
    let mut seen: Vec<&str> = Vec::new();
    for row in &rows {
        let nth = seen.iter().filter(|f| **f == row.family).count();
        seen.push(row.family);
        let item_session = match nth % 3 {
            0 => 1,
            1 => 2,
            _ => 3,
        };
        if item_session != session {
            continue;
        }
        let takes = match session {
            1 => 3,
            2 => 2,
            _ => 1,
        };
        for take in 1..=takes {
            out.push(PromptSpec {
                id: format!("s{session}-{}-{nth}-t{take}", row.family),
                session,
                prompt: row.text.into(),
                expected_text: row.text.into(),
                expected_intent: row.intent.into(),
                kind: "command".into(),
                takes,
            });
        }
    }
}

fn push_tools(out: &mut Vec<PromptSpec>, session: u8, contacts: &[String], aliases: &[String]) {
    for (i, name) in contacts.iter().enumerate() {
        if name.eq_ignore_ascii_case("me") {
            continue;
        }
        let item_session = ((i % 3) + 1) as u8;
        if item_session != session {
            continue;
        }
        let rows = [
            (format!("Send a Google Meet link to {name}"), "meet"),
            (format!("Make a Google Meet and send the link to {name}"), "meet"),
            (format!("Send this link to {name} on WhatsApp"), "whatsapp"),
        ];
        for (n, (text, intent)) in rows.into_iter().enumerate() {
            out.push(PromptSpec {
                id: format!("s{session}-tool-{i}-{n}"),
                session,
                prompt: text.clone(),
                expected_text: text,
                expected_intent: intent.into(),
                kind: "tool".into(),
                takes: 1,
            });
        }
    }
    for (i, alias) in aliases.iter().enumerate() {
        let item_session = ((i % 3) + 1) as u8;
        if item_session != session {
            continue;
        }
        let text = format!("run {alias}");
        out.push(PromptSpec {
            id: format!("s{session}-alias-{i}"),
            session,
            prompt: text.clone(),
            expected_text: text,
            expected_intent: "workflow".into(),
            kind: "tool".into(),
            takes: 1,
        });
    }
}

fn push_negatives(out: &mut Vec<PromptSpec>, session: u8) {
    const ROWS: &[&str] = &[
        "why does my monitor turn off when I play games",
        "what is a mutex",
        "explain borrowing in rust",
        "how does TCP work",
        "what is the capital of Japan",
        "tell me a short joke",
        "is the sky blue today",
        "what does latency mean",
        "how many bits in a byte",
        "what is a neural net",
        "why is my fan loud",
        "which zone is Tokyo in",
        "how do I center a div",
        "what is an API",
        "who wrote Hamlet",
        "what is photosynthesis",
        "how far is the moon",
        "what is a hash map",
        "why do we use git",
        "what is unicode",
        "how does Wi Fi work",
        "what is a compiler",
        "why are types useful",
        "what is a pointer",
        "how do batteries work",
        "what is JSON",
        "why is rust safe",
        "what is a thread",
        "how does a speaker work",
        "what is entropy",
        "why is the ocean salty",
        "what is a kernel",
        "how do caches work",
        "what is latency hiding",
        "why use a GPU",
        "what is a sample rate",
        "how do microphones work",
        "what is a phoneme",
        "why is English hard",
        "what is a token",
        "how do embeddings work",
        "what is overfitting",
        "why split train and test",
        "what is a false positive",
        "how do I stay focused",
        "what is a study plan",
        "why is sleep useful",
        "what is a chord",
        "how does reverb work",
        "what is a playlist",
        "why does this song sound loud",
        "what is a calendar",
        "how do reminders work",
        "what is a deadline in school",
        "why do exams exist",
        "what is a PDF",
        "how do links work",
        "what is a browser",
        "why do meetings run long",
        "what is a contact list",
    ];
    for (i, text) in ROWS.iter().enumerate() {
        let item_session = ((i % 3) + 1) as u8;
        if item_session != session {
            continue;
        }
        out.push(PromptSpec {
            id: format!("s{session}-neg-{i}"),
            session,
            prompt: (*text).into(),
            expected_text: (*text).into(),
            expected_intent: "none".into(),
            kind: "negative".into(),
            takes: 1,
        });
    }
}

fn push_specials(out: &mut Vec<PromptSpec>, session: u8) {
    if session == 1 {
        for i in 0..4 {
            out.push(empty_clip(session, &format!("silence-{i}"), "Stay silent until the stop.", "silence"));
        }
    }
    if session == 2 {
        for i in 0..4 {
            out.push(empty_clip(
                session,
                &format!("noise-{i}"),
                "Do not speak. Leave the room noise running.",
                "noise",
            ));
        }
        for i in 0..3 {
            out.push(empty_clip(session, &format!("short-{i}"), "Tap the key and stop immediately.", "short"));
        }
    }
    if session == 3 {
        let text = "This is a short dictation about studying at the desk without asking Kuro to do anything.";
        out.push(PromptSpec {
            id: format!("s{session}-dict-0"),
            session,
            prompt: text.into(),
            expected_text: text.into(),
            expected_intent: "none".into(),
            kind: "dictation".into(),
            takes: 1,
        });
    }
}

fn empty_clip(session: u8, id: &str, prompt: &str, kind: &str) -> PromptSpec {
    PromptSpec {
        id: format!("s{session}-{id}"),
        session,
        prompt: prompt.into(),
        expected_text: String::new(),
        expected_intent: "none".into(),
        kind: kind.into(),
        takes: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_cover_commands_without_personal_names() {
        let a = catalog_for_session(1, &[], &[]);
        let b = catalog_for_session(2, &[], &[]);
        let c = catalog_for_session(3, &[], &[]);
        let n = catalog_for_session(1, &["A".into()], &["hello workflow".into()]).len()
            + catalog_for_session(2, &["A".into()], &["hello workflow".into()]).len()
            + catalog_for_session(3, &["A".into()], &["hello workflow".into()]).len();
        assert!(n >= 320, "catalog {n}");
        for session in 1..=3 {
            for prompt in catalog_for_session(session, &[], &[]) {
                if prompt.kind == "command" || prompt.kind == "negative" {
                    let got = kuro_router::classify_intent(&prompt.expected_text);
                    assert_eq!(got, prompt.expected_intent, "{}", prompt.prompt);
                }
            }
        }
        assert!(a.iter().any(|p| p.expected_intent == "power_on"));
        assert!(b.iter().any(|p| p.kind == "negative"));
        assert!(c.iter().any(|p| p.kind == "command"));
        assert!(a.iter().all(|p| !p.prompt.chars().any(|ch| ch.is_ascii_digit() && p.prompt.contains("91"))));
    }
}
