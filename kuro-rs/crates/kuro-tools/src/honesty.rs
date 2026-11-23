//! Chat must never invent tool success. Actuation only comes from tool replies.

use crate::n8n::normalize;

fn low(s: &str) -> String {
    normalize(s)
}

/// User asked for something that only a tool can do (Meet, WhatsApp, workflow).
pub fn wants_tool_actuation(user: &str) -> bool {
    let t = low(user);
    if t.is_empty() {
        return false;
    }
    if crate::meet::looks_like_meet_request(user) {
        return true;
    }
    if t.contains("whatsapp") {
        return true;
    }
    if (t.contains("send ") || t.starts_with("message ") || t.starts_with("text "))
        && (t.contains(" link") || t.contains("clipboard") || t.contains("this "))
    {
        return true;
    }
    if t.contains("run ") && (t.contains("workflow") || t.contains("n8n")) {
        return true;
    }
    false
}

/// Model text that pretends a side effect already happened.
pub fn claims_actuation_success(reply: &str) -> bool {
    let t = low(reply);
    if t.is_empty() {
        return false;
    }
    let hits = [
        "i'll send",
        "i will send",
        "i am sending",
        "i'm sending",
        "sending the",
        "sending that",
        "sent the",
        "sent that",
        "sent it",
        "got it sending",
        "opening the meet",
        "opening your",
        "opening a meet",
        "i'm already opening",
        "i am opening",
        "i'll open",
        "i will open",
        "joining now",
        "joined the meet",
        "you can join",
        "meet link for",
        "done you can",
        "... done",
        "done.",
        "created a meet",
        "created the meet",
        "shared the link",
        "on its way",
    ];
    hits.iter().any(|h| t.contains(h))
}

pub fn honest_tool_miss(user: &str) -> String {
    if crate::meet::looks_like_meet_request(user) {
        return "I could not run that Meet command. No browser opened and nothing was sent. Say send a Google Meet link to a contact name."
            .into();
    }
    if low(user).contains("whatsapp") || low(user).contains("send ") {
        return "I could not run that WhatsApp send. Nothing was delivered.".into();
    }
    "I could not run that action. No tool completed it.".into()
}

/// If chat claimed success for an actuation request, replace with an honest miss.
pub fn scrub_chat_reply(user: &str, reply: &str) -> String {
    if wants_tool_actuation(user) && claims_actuation_success(reply) {
        return honest_tool_miss(user);
    }
    reply.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_kills_fake_meet_send() {
        let user = "Send a Google Meet link to Siya.";
        let fake = "Sure, opening your recent Google Meet link... done. You can join the meeting now.";
        let out = scrub_chat_reply(user, fake);
        assert!(out.contains("could not run"), "got: {out}");
        assert!(!out.to_ascii_lowercase().contains("done"));
    }

    #[test]
    fn scrub_leaves_normal_chat() {
        let out = scrub_chat_reply("what's the weather vibe", "It feels calm tonight.");
        assert_eq!(out, "It feels calm tonight.");
    }

    #[test]
    fn meet_request_detected() {
        assert!(wants_tool_actuation("send a google meet link td siya"));
        assert!(claims_actuation_success("Got it, sending the Google Meet link to Nordzia."));
    }
}
