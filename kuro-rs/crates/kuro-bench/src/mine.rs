use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use kuro_router::{classify_intent, normalize};

const COMMANDISH: &[&str] = &[
    "desk",
    "light",
    "lights",
    "monitor",
    "screen",
    "power",
    "daddy",
    "music",
    "song",
    "pause",
    "news",
    "inbox",
    "mail",
    "deadline",
    "meet",
    "whatsapp",
    "workflow",
    "cancel",
    "good night",
    "goodnight",
];

/// Command-shaped line that the pure classifier did not accept.
pub fn command_attempt_miss(text: &str) -> bool {
    let norm = normalize(text);
    if norm.is_empty() || classify_intent(text) != "none" {
        return false;
    }
    COMMANDISH.iter().any(|token| norm.contains(token))
}

pub fn mine_stt_lines(lines: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let line = line.as_ref();
        let Some(text) = line.split("stt text=").nth(1) else {
            continue;
        };
        let text = text.trim().trim_matches('`').trim();
        if text.is_empty() || !command_attempt_miss(text) {
            continue;
        }
        if !out.iter().any(|seen: &String| seen == text) {
            out.push(text.to_string());
        }
    }
    out
}

pub fn mine_logs(paths: &[impl AsRef<Path>], dest: &Path) -> Result<usize, String> {
    let mut lines = Vec::new();
    for path in paths {
        let path = path.as_ref();
        if !path.exists() {
            continue;
        }
        let file = File::open(path).map_err(|err| err.to_string())?;
        for line in BufReader::new(file).lines() {
            lines.push(line.map_err(|err| err.to_string())?);
        }
    }
    let mined = mine_stt_lines(lines);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = File::create(dest).map_err(|err| err.to_string())?;
    for row in &mined {
        writeln!(file, "{row}").map_err(|err| err.to_string())?;
    }
    Ok(mined.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mines_unmatched_desk_attempt() {
        let lines = ["stt text=`bob the desk`", "stt text=`desk on`", "stt text=`hello there`"];
        let got = mine_stt_lines(lines);
        assert_eq!(got, vec!["bob the desk".to_string()]);
    }
}
