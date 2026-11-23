use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde_json::json;

use crate::adapters::{desk_scene_reply, join_meet_reply, meet_reply, music_op_reply, whatsapp_reply};

use super::PhraseResult;
use super::matchers::{
    is_cancel, is_daddys_home, is_date, is_goodnight, is_help, is_identity, is_inbox, is_join_meet, is_meet, is_news,
    is_pause, is_power_off, is_power_on, is_resume, is_stop_music, is_thanks, is_time, is_whatsapp, is_workflow,
    is_workflow_hint,
};

/// Debounce daddy-home across rapid repeats. Cleared only by cooldown, not when
/// music/plug threads finish (those return instantly on headless CI and used to
/// re-arm the latch before the next phrase in the same test).
static HOME_COOLDOWN_UNTIL: AtomicU64 = AtomicU64::new(0);
const HOME_COOLDOWN_SECS: u64 = 45;

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn try_handle(text: &str) -> Option<PhraseResult> {
    catch_unwind(AssertUnwindSafe(|| try_handle_inner(text))).unwrap_or_else(|_| {
        Some(PhraseResult {
            reply: "Something went wrong with that phrase.".into(),
            skip_tts: false,
            force_fast_tts: true,
        })
    })
}

fn try_handle_inner(text: &str) -> Option<PhraseResult> {
    if text.trim().is_empty() {
        return None;
    }
    let t = super::normalize(text);
    match crate::semantic::route_semantic(&t) {
        Some(crate::semantic::RouteDecision::Llm) => return None,
        Some(crate::semantic::RouteDecision::Tool(intent)) => {
            if let Some(result) = execute_intent(intent, &t) {
                return Some(result);
            }
        }
        None => {}
    }
    legacy_try_handle(&t)
}

pub(crate) fn fast(reply: String) -> PhraseResult {
    PhraseResult { reply, skip_tts: false, force_fast_tts: true }
}

fn spoken_intent_reply(intent_name: &str, fallback: String) -> String {
    kuro_features::spoken_for_intent(intent_name).unwrap_or(fallback)
}

fn execute_intent(intent: crate::semantic::Intent, t: &str) -> Option<PhraseResult> {
    use crate::semantic::Intent;
    Some(match intent {
        Intent::Cancel => fast(spoken_intent_reply("Cancel", "Alright.".into())),
        Intent::Thanks => fast(spoken_intent_reply("Thanks", "Anytime.".into())),
        Intent::Help => fast(spoken_intent_reply(
            "Help",
            "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. Say run hello workflow to test n8n.".into(),
        )),
        Intent::Identity => {
            fast(spoken_intent_reply("Identity", "Kuro. Local assistant. Phrases first, model last.".into()))
        }
        Intent::Time => fast(spoken_intent_reply("Time", clock_reply())),
        Intent::Date => fast(date_reply()),
        Intent::PauseMusic => fast(music_op_reply("pause")),
        Intent::ResumeMusic => fast(music_op_reply("resume")),
        Intent::StopMusic => fast(music_op_reply("stop")),
        Intent::GoodNight => {
            let _ = music_op_reply("stop");
            fast(desk_scene_reply("good_night"))
        }
        Intent::DaddyHome => {
            if !fire_daddys_home() {
                return Some(fast("Already on it, sir.".into()));
            }
            fast("Welcome home, sir.".into())
        }
        Intent::PowerOff => {
            let _ = music_op_reply("stop");
            fast(desk_scene_reply("power_off"))
        }
        Intent::PowerOn => fast(desk_scene_reply("power_on")),
        Intent::GetNews => {
            let topic = news_topic(t);
            let raw = kuro_tools::execute_tool("get_news", &json!({"topic": topic}));
            fast(news_to_speech(&raw))
        }
        Intent::MakeMeet => {
            // Phrase match required so semantic near-misses never create a live Meet.
            if is_join_meet(t) {
                fast(join_meet_reply(t))
            } else if is_meet(t) {
                fast(meet_reply(t))
            } else {
                return None;
            }
        }
        Intent::JoinMeet => {
            // Never create from JoinMeet. Create cooldown must not block a join.
            if is_whatsapp(t) { fast(whatsapp_reply(t)) } else { fast(join_meet_reply(t)) }
        }
        Intent::CheckInbox => {
            let mode = inbox_mode(t);
            let raw = kuro_tools::execute_tool("check_inbox", &json!({"mode": mode}));
            fast(inbox_to_speech(&raw))
        }
        Intent::SendWhatsapp => fast(whatsapp_reply(t)),
        Intent::RunWorkflow => fast(workflow_reply(t)),
        Intent::ReadScreen => {
            let raw = kuro_tools::execute_tool("read_screen", &json!({}));
            fast(raw)
        }
        Intent::ConversationalLlm => return None,
    })
}

fn legacy_try_handle(t: &str) -> Option<PhraseResult> {
    if is_cancel(t) {
        return Some(fast(spoken_intent_reply("Cancel", "Alright.".into())));
    }
    if is_thanks(t) {
        return Some(fast(spoken_intent_reply("Thanks", "Anytime.".into())));
    }
    if is_help(t) {
        return Some(fast(spoken_intent_reply(
            "Help",
            "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. Say run hello workflow to test n8n.".into(),
        )));
    }
    if is_identity(t) {
        return Some(fast(spoken_intent_reply("Identity", "Kuro. Local assistant. Phrases first, model last.".into())));
    }
    if is_time(t) {
        return Some(fast(spoken_intent_reply("Time", clock_reply())));
    }
    if is_date(t) {
        return Some(fast(date_reply()));
    }
    if is_pause(t) {
        return Some(fast(music_op_reply("pause")));
    }
    if is_resume(t) {
        return Some(fast(music_op_reply("resume")));
    }
    if is_stop_music(t) {
        return Some(fast(music_op_reply("stop")));
    }
    if is_goodnight(t) {
        let _ = music_op_reply("stop");
        return Some(fast(desk_scene_reply("good_night")));
    }
    if is_daddys_home(t) {
        if !fire_daddys_home() {
            return Some(fast("Already on it, sir.".into()));
        }
        return Some(fast("Welcome home, sir.".into()));
    }
    if is_power_off(t) {
        let _ = music_op_reply("stop");
        return Some(fast(desk_scene_reply("power_off")));
    }
    if is_power_on(t) {
        return Some(fast(desk_scene_reply("power_on")));
    }
    if is_news(t) {
        let topic = news_topic(t);
        let raw = kuro_tools::execute_tool("get_news", &json!({"topic": topic}));
        return Some(fast(news_to_speech(&raw)));
    }
    if is_join_meet(t) {
        return Some(fast(join_meet_reply(t)));
    }
    if is_meet(t) {
        return Some(fast(meet_reply(t)));
    }
    if is_inbox(t) {
        let mode = inbox_mode(t);
        let raw = kuro_tools::execute_tool("check_inbox", &json!({"mode": mode}));
        return Some(fast(inbox_to_speech(&raw)));
    }
    if is_whatsapp(t) {
        return Some(fast(whatsapp_reply(t)));
    }
    if is_workflow(t) {
        return Some(fast(workflow_reply(t)));
    }
    if is_workflow_hint(t) {
        return Some(fast("I don't know that workflow. Check the local n8n registry.".into()));
    }
    None
}

fn fire_daddys_home() -> bool {
    let now = unix_now();
    let until = HOME_COOLDOWN_UNTIL.load(Ordering::SeqCst);
    if now < until {
        return false;
    }
    HOME_COOLDOWN_UNTIL.store(now.saturating_add(HOME_COOLDOWN_SECS), Ordering::SeqCst);
    std::thread::spawn(|| {
        let _ = kuro_tools::execute_tool("control_smart_plug", &json!({"scene": "daddy_home"}));
    });
    std::thread::spawn(|| {
        let _ = kuro_tools::music::play_should_i_stay_or_should_i_go();
    });
    true
}

fn clock_reply() -> String {
    Local::now().format("It's %I:%M %p.").to_string().trim_start_matches('0').replace(" 0", " ")
}
fn date_reply() -> String {
    Local::now().format("Today is %A, %B %d.").to_string().replace(" 0", " ")
}
fn news_topic(t: &str) -> &'static str {
    for key in ["tech", "world", "business", "ai", "sports"] {
        if t.contains(key) {
            return key;
        }
    }
    "general"
}
fn news_to_speech(raw: &str) -> String {
    if raw.is_empty() || raw.to_lowercase().contains("unable") {
        return "I could not fetch the news right now.".into();
    }
    let mut lines = Vec::new();
    for line in raw.lines() {
        let mut line = line.trim().trim_start_matches('-').trim().to_string();
        if line.is_empty() || line.to_lowercase().starts_with("latest") {
            continue;
        }
        if let Some(idx) = line.find("] ") {
            line = line[idx + 2..].to_string();
        }
        let title = if let Some((head, _)) = line.split_once(':') { head.trim().to_string() } else { line };
        if !title.is_empty() {
            lines.push(title);
        }
        if lines.len() >= 3 {
            break;
        }
    }
    match lines.len() {
        0 => "No headlines came through.".into(),
        1 => format!("Top story: {}.", lines[0]),
        _ => format!(
            "Here are the top headlines. {}. Next: {}. And: {}.",
            lines[0],
            lines[1],
            lines.get(2).map(|s| s.as_str()).unwrap_or("")
        ),
    }
}
fn inbox_mode(t: &str) -> &'static str {
    if t.contains("deadline") {
        "deadlines"
    } else if t.contains("urgent") || t.contains("important") {
        "urgent"
    } else {
        "summary"
    }
}
fn inbox_to_speech(raw: &str) -> String {
    let text = raw.trim();
    if text.is_empty() {
        return "I couldn't reach the cloud inbox.".into();
    }
    if text.chars().count() > 280 {
        format!("{}...", text.chars().take(277).collect::<String>().trim_end())
    } else {
        text.to_string()
    }
}
fn speakable(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.to_lowercase().starts_with("success:") {
        text = text.split_once(':').map(|(_, rest)| rest.trim().to_string()).unwrap_or(text);
    }
    if text.is_empty() { "Done, sir.".into() } else { text }
}
fn workflow_reply(t: &str) -> String {
    let Some((_, args)) = kuro_tools::match_registered_phrase(t).filter(|(tool, _)| tool.name == "run_workflow") else {
        return "I don't know that workflow.".into();
    };
    speakable(&kuro_tools::execute_tool("run_workflow", &args))
}
