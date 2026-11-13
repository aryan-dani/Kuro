use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use regex::Regex;
use serde_json::json;

/// Debounce daddy-home across rapid repeats. Cleared only by cooldown — not when
/// music/plug threads finish (those return instantly on headless CI and used to
/// re-arm the latch before the next phrase in the same test).
static HOME_COOLDOWN_UNTIL: AtomicU64 = AtomicU64::new(0);
const HOME_COOLDOWN_SECS: u64 = 45;

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Clone, Debug)]
pub struct PhraseResult {
    pub reply: String,
    pub skip_tts: bool,
    pub force_fast_tts: bool,
}

pub fn normalize(text: &str) -> String {
    let mut text = text.to_lowercase();
    const PRE: &[(&str, &str)] = &[
        ("daddy's", "daddy"),
        ("daddys", "daddy"),
        ("dadi", "daddy"),
        ("wake up", "wakeup"),
        ("wake-up", "wakeup"),
        ("woke up", "wakeup"),
        ("good night", "goodnight"),
        ("his home", "daddy home"),
        ("in his home", "daddy home"),
        ("at his home", "daddy home"),
        ("in's home", "daddy home"),
        ("what's", "whats"),
        ("what s", "whats"),
        ("moniter", "monitor"),
        ("monitors", "monitor"),
        ("extention", "extension"),
        ("monitor of", "monitor off"),
        ("screen of", "screen off"),
        ("display of", "display off"),
        ("desk of", "desk off"),
        ("lights of", "lights off"),
        ("light of", "light off"),
        ("plug of", "plug off"),
        ("power of", "power off"),
        ("board of", "board off"),
        ("let's off", "desk off"),
        ("lets off", "desk off"),
        ("let s off", "desk off"),
        ("next off", "desk off"),
        ("bob it off", "power off"),
        ("urgent man", "urgent mail"),
        ("urgent men", "urgent mail"),
    ];
    for (from, to) in PRE {
        text = text.replace(from, to);
    }
    let mut cleaned = String::new();
    let mut space = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            cleaned.push(ch);
            space = false;
        } else if !space && !cleaned.is_empty() {
            cleaned.push(' ');
            space = true;
        }
    }
    let mut text = cleaned.trim().to_string();
    for (from, to) in
        [("lets off", "desk off"), ("let s off", "desk off"), ("next off", "desk off"), ("bob it off", "power off")]
    {
        if text.contains(from) {
            text = text.replace(from, to);
        }
    }
    text
}

pub fn has_word(text: &str, word: &str) -> bool {
    let pattern = format!(r"\b{}\b", regex::escape(word));
    Regex::new(&pattern).map(|re| re.is_match(text)).unwrap_or(false)
}

pub fn looks_like_daddys_home(text: &str) -> bool {
    is_daddys_home(&normalize(text))
}

pub fn matches(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let t = normalize(text);
    is_cancel(&t)
        || is_thanks(&t)
        || is_help(&t)
        || is_identity(&t)
        || is_time(&t)
        || is_date(&t)
        || is_pause(&t)
        || is_resume(&t)
        || is_stop_music(&t)
        || is_goodnight(&t)
        || is_daddys_home(&t)
        || is_power_off(&t)
        || is_power_on(&t)
        || is_news(&t)
        || is_inbox(&t)
        || is_meet(&t)
        || is_whatsapp(&t)
        || is_workflow(&t)
        || is_workflow_hint(&t)
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
    let t = normalize(text);
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

fn fast(reply: String) -> PhraseResult {
    PhraseResult { reply, skip_tts: false, force_fast_tts: true }
}

fn execute_intent(intent: crate::semantic::Intent, t: &str) -> Option<PhraseResult> {
    use crate::semantic::Intent;
    Some(match intent {
        Intent::Cancel => fast("Alright.".into()),
        Intent::Thanks => fast("Anytime.".into()),
        Intent::Help => fast(
            "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. Say run hello workflow to test n8n.".into(),
        ),
        Intent::Identity => fast("Kuro. Local assistant. Phrases first, model last.".into()),
        Intent::Time => fast(clock_reply()),
        Intent::Date => fast(date_reply()),
        Intent::PauseMusic => fast(kuro_tools::music::pause_song()),
        Intent::ResumeMusic => fast(kuro_tools::music::resume_song()),
        Intent::StopMusic => fast(kuro_tools::music::stop_song()),
        Intent::GoodNight => {
            let _ = kuro_tools::music::stop_song();
            fast(power("good_night", "Desk off. Good night, sir."))
        }
        Intent::DaddyHome => {
            if !fire_daddys_home() {
                return Some(fast("Already on it, sir.".into()));
            }
            fast("Welcome home, sir.".into())
        }
        Intent::PowerOff => {
            let _ = kuro_tools::music::stop_song();
            fast(power("power_off", "Desk off."))
        }
        Intent::PowerOn => fast(power("power_on", "Desk on.")),
        Intent::GetNews => {
            let topic = news_topic(t);
            let raw = kuro_tools::execute_tool("get_news", &json!({"topic": topic}));
            fast(news_to_speech(&raw))
        }
        Intent::MakeMeet => fast(meet_reply(t)),
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
        return Some(fast("Alright.".into()));
    }
    if is_thanks(t) {
        return Some(fast("Anytime.".into()));
    }
    if is_help(t) {
        return Some(fast(
            "Desk, music, news, inbox, Meet, WhatsApp, and named workflows. Say run hello workflow to test n8n.".into(),
        ));
    }
    if is_identity(t) {
        return Some(fast("Kuro. Local assistant. Phrases first, model last.".into()));
    }
    if is_time(t) {
        return Some(fast(clock_reply()));
    }
    if is_date(t) {
        return Some(fast(date_reply()));
    }
    if is_pause(t) {
        return Some(fast(kuro_tools::music::pause_song()));
    }
    if is_resume(t) {
        return Some(fast(kuro_tools::music::resume_song()));
    }
    if is_stop_music(t) {
        return Some(fast(kuro_tools::music::stop_song()));
    }
    if is_goodnight(t) {
        let _ = kuro_tools::music::stop_song();
        return Some(fast(power("good_night", "Desk off. Good night, sir.")));
    }
    if is_daddys_home(t) {
        if !fire_daddys_home() {
            return Some(fast("Already on it, sir.".into()));
        }
        return Some(fast("Welcome home, sir.".into()));
    }
    if is_power_off(t) {
        let _ = kuro_tools::music::stop_song();
        return Some(fast(power("power_off", "Desk off.")));
    }
    if is_power_on(t) {
        return Some(fast(power("power_on", "Desk on.")));
    }
    if is_news(t) {
        let topic = news_topic(t);
        let raw = kuro_tools::execute_tool("get_news", &json!({"topic": topic}));
        return Some(fast(news_to_speech(&raw)));
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

pub fn is_cancel(t: &str) -> bool {
    matches!(t, "cancel" | "never mind" | "nevermind" | "forget it" | "stop" | "abort")
}
pub fn is_thanks(t: &str) -> bool {
    ["thank you", "thanks", "cheers", "appreciate it"].iter().any(|p| t.contains(p))
}
pub fn is_help(t: &str) -> bool {
    // Exact / short help only. Do not substring-match "help me" inside real questions
    // like "can you help me build an LLM".
    matches!(
        t,
        "help"
            | "commands"
            | "help me"
            | "what can you do"
            | "your commands"
            | "what do you support"
            | "list commands"
            | "capabilities"
            | "can you help"
            | "can you help me"
    ) || (t.split_whitespace().count() <= 6
        && ["what can you do", "list commands", "your commands", "what do you support", "capabilities"]
            .iter()
            .any(|p| t.contains(p)))
}
pub fn is_identity(t: &str) -> bool {
    ["who are you", "what are you", "your name", "whats your name"].iter().any(|p| t.contains(p))
}
pub fn is_date(t: &str) -> bool {
    t == "date"
        || t == "the date"
        || ["what day", "whats the date", "what date", "todays date", "current date", "what is today"]
            .iter()
            .any(|p| t.contains(p))
}
pub fn is_time(t: &str) -> bool {
    if is_date(t) && !t.contains("time") {
        return false;
    }
    t == "time"
        || t == "the time"
        || ["what time", "whats the time", "current time", "tell me the time", "time is it"]
            .iter()
            .any(|p| t.contains(p))
}
pub fn is_pause(t: &str) -> bool {
    if !t.contains("pause") {
        return false;
    }
    ["song", "music", "track", "audio", "media"].iter().any(|w| t.contains(w))
        || matches!(t, "pause" | "pause it" | "pause please" | "pause that")
}
pub fn is_resume(t: &str) -> bool {
    [
        "resume",
        "unpause",
        "continue the song",
        "continue music",
        "play the song",
        "play music",
        "keep playing",
        "resume music",
        "resume song",
    ]
    .iter()
    .any(|p| t.contains(p))
}
pub fn is_stop_music(t: &str) -> bool {
    ["stop", "close", "shut off", "shut down", "kill"].iter().any(|w| t.contains(w))
        && ["song", "music", "track", "clash", "media"].iter().any(|w| t.contains(w))
}
pub fn is_goodnight(t: &str) -> bool {
    t.contains("goodnight") || (t.contains("good") && t.contains("night")) || matches!(t, "night" | "sleep" | "bedtime")
}
pub fn is_daddys_home(t: &str) -> bool {
    let home = t.contains("home");
    let daddy = t.contains("daddy");
    let wake = t.contains("wakeup") || t.contains("wake");
    (wake && home) || (daddy && home) || (wake && daddy)
}
fn desk_keywords(t: &str) -> bool {
    ["light", "lights", "lamp", "plug", "charger", "monitor", "screen", "display", "desk", "board", "extension"]
        .iter()
        .any(|w| has_word(t, w))
}
fn power_command_shaped(t: &str) -> bool {
    // Imperative desk control — not "why does my monitor turn off".
    !crate::semantic::looks_like_question(t)
}
pub fn is_power_off(t: &str) -> bool {
    if !power_command_shaped(t) {
        return false;
    }
    if has_word(t, "on") && !has_word(t, "off") {
        return false;
    }
    if [
        "lights off",
        "light off",
        "plug off",
        "switch off",
        "power off",
        "power down",
        "monitor off",
        "screen off",
        "desk off",
        "display off",
    ]
    .iter()
    .any(|p| t.contains(p))
    {
        return true;
    }
    if has_word(t, "off") && desk_keywords(t) {
        return true;
    }
    t.contains("turn off") || t.contains("switch off")
}
pub fn is_power_on(t: &str) -> bool {
    if !power_command_shaped(t) {
        return false;
    }
    if has_word(t, "off") {
        return false;
    }
    if ["lights on", "light on", "plug on", "switch on", "power on", "monitor on", "screen on", "desk on", "display on"]
        .iter()
        .any(|p| t.contains(p))
    {
        return true;
    }
    if has_word(t, "on") && desk_keywords(t) {
        return true;
    }
    t.contains("turn on") || t.contains("switch on")
}
pub fn is_inbox(t: &str) -> bool {
    [
        "urgent mail",
        "urgent email",
        "any urgent",
        "inbox check",
        "check inbox",
        "check my inbox",
        "check my email",
        "check my mail",
        "check email",
        "check mail",
        "deadlines today",
        "any deadlines",
        "my deadlines",
        "important email",
        "important mail",
        "important emails",
        "missed email",
        "missed mail",
    ]
    .iter()
    .any(|p| t.contains(p))
}
pub fn is_news(t: &str) -> bool {
    if is_inbox(t) {
        return false;
    }
    [
        "news",
        "headlines",
        "whats happening",
        "whats the news",
        "what is the news",
        "brief me",
        "any updates",
        "latest news",
        "give me the news",
        "read the news",
    ]
    .iter()
    .any(|p| t.contains(p))
}
pub fn is_meet(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "make_meet")
}
pub fn is_whatsapp(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "send_whatsapp")
}
pub fn is_workflow(t: &str) -> bool {
    kuro_tools::match_registered_phrase(t).is_some_and(|(tool, _)| tool.name == "run_workflow")
}
pub fn is_workflow_hint(t: &str) -> bool {
    t.contains("n8n")
        || t.contains("workflow")
        || (t.contains("automation") && ["run", "start", "trigger", "launch"].iter().any(|v| t.contains(v)))
}
pub fn is_music_only(t: &str) -> bool {
    is_pause(t) || is_resume(t) || is_stop_music(t)
}
pub fn is_hub_desk(t: &str) -> bool {
    is_daddys_home(t) || is_goodnight(t) || is_power_on(t) || is_power_off(t) || is_news(t)
}

fn power(scene: &str, speak_ok: &str) -> String {
    let raw = kuro_tools::execute_tool("control_smart_plug", &json!({"scene": scene}));
    if raw.is_empty() {
        return "I couldn't reach the plug.".into();
    }
    let low = raw.to_lowercase();
    if low.starts_with("success") {
        return speak_ok.into();
    }
    if ["can't reach", "unable", "error", "missing", "offline"].iter().any(|k| low.contains(k)) {
        return if raw.len() < 160 { raw } else { "I couldn't reach the plug.".into() };
    }
    speak_ok.into()
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
fn meet_reply(t: &str) -> String {
    let args = kuro_tools::match_registered_phrase(t)
        .filter(|(tool, _)| tool.name == "make_meet")
        .map(|(_, args)| args)
        .unwrap_or_else(|| json!({"contact": ""}));
    speakable(&kuro_tools::execute_tool("make_meet", &args))
}
fn whatsapp_reply(t: &str) -> String {
    let Some((_, args)) = kuro_tools::match_registered_phrase(t).filter(|(tool, _)| tool.name == "send_whatsapp")
    else {
        return "I don't know who to message.".into();
    };
    speakable(&kuro_tools::execute_tool("send_whatsapp", &args))
}
fn workflow_reply(t: &str) -> String {
    let Some((_, args)) = kuro_tools::match_registered_phrase(t).filter(|(tool, _)| tool.name == "run_workflow") else {
        return "I don't know that workflow.".into();
    };
    speakable(&kuro_tools::execute_tool("run_workflow", &args))
}

#[derive(Clone, Debug)]
pub struct RouteResult {
    pub reply: String,
    pub skip_tts: bool,
    pub force_fast: bool,
    pub start_ollama: bool,
}

pub fn route_command(
    text: &str,
    live: bool,
    hub_handled: Option<Result<Option<serde_json::Value>, ()>>,
    llm_mode: &str,
    ollama_up: bool,
) -> RouteResult {
    if !live {
        return RouteResult { reply: String::new(), skip_tts: true, force_fast: true, start_ollama: false };
    }
    if let Some(local) = try_handle(text) {
        return RouteResult {
            reply: local.reply,
            skip_tts: local.skip_tts,
            force_fast: local.force_fast_tts,
            start_ollama: false,
        };
    }
    if let Some(result) = hub_handled {
        if !live {
            return RouteResult { reply: String::new(), skip_tts: true, force_fast: true, start_ollama: false };
        }
        match result {
            Err(()) => {
                let t = normalize(text);
                if is_music_only(&t)
                    && let Some(local) = try_handle(text)
                {
                    return RouteResult {
                        reply: local.reply,
                        skip_tts: local.skip_tts,
                        force_fast: local.force_fast_tts,
                        start_ollama: false,
                    };
                }
                if is_hub_desk(&t) {
                    return RouteResult {
                        reply: "I can't reach the hub.".into(),
                        skip_tts: false,
                        force_fast: false,
                        start_ollama: false,
                    };
                }
            }
            Ok(Some(value)) if value.get("duplicate").and_then(|v| v.as_bool()).unwrap_or(false) => {
                return RouteResult { reply: String::new(), skip_tts: true, force_fast: false, start_ollama: false };
            }
            Ok(Some(value)) if value.get("handled").and_then(|v| v.as_bool()).unwrap_or(false) => {
                if let Some(actions) = value.get("client_actions").and_then(|v| v.as_array()) {
                    for action in actions {
                        kuro_tools::music::apply_action(action);
                    }
                }
                return RouteResult {
                    reply: value.get("reply").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    skip_tts: false,
                    force_fast: false,
                    start_ollama: false,
                };
            }
            _ => {}
        }
    }
    if let Some(local) = try_handle(text) {
        return RouteResult {
            reply: local.reply,
            skip_tts: local.skip_tts,
            force_fast: local.force_fast_tts,
            start_ollama: false,
        };
    }
    if llm_mode == "off" {
        return RouteResult {
            reply: "I don't have a phrase for that yet. Desk, news, music, meet, WhatsApp, and workflows work offline."
                .into(),
            skip_tts: false,
            force_fast: true,
            start_ollama: false,
        };
    }
    if !ollama_up {
        return RouteResult {
            reply: "Neural engine is off. Please wait while I start it.".into(),
            skip_tts: false,
            force_fast: true,
            start_ollama: true,
        };
    }
    RouteResult { reply: String::new(), skip_tts: false, force_fast: false, start_ollama: true }
}

pub fn prefer_user_text(live: &str, final_text: &str) -> String {
    kuro_core::prefer_transcript(live, final_text, matches(live), matches(final_text))
}
