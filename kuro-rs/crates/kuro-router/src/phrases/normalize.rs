use regex::Regex;

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
