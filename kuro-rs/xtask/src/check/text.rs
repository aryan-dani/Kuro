use std::path::Path;

use crate::paths::{git_lines, normalize_rel, repo_root};

const SCAN_EXT: &[&str] = &[".rs", ".cs", ".xaml", ".py", ".ps1", ".md", ".json", ".toml", ".yml", ".yaml"];

pub fn scan_line(line: &str, rel: &str, line_no: usize, errors: &mut Vec<String>) {
    for (idx, ch) in line.char_indices() {
        if is_emoji(ch) {
            errors.push(format!("{rel}:{}:{} emoji U+{:04X}", line_no, idx + 1, ch as u32));
        }
        if ch == '\u{2014}' {
            errors.push(format!("{rel}:{}:{} em dash U+2014", line_no, idx + 1));
        }
        if ch == '\u{2013}' {
            errors.push(format!("{rel}:{}:{} en dash U+2013", line_no, idx + 1));
        }
    }
}

fn is_emoji(ch: char) -> bool {
    let cp = ch as u32;
    (0x2600..=0x27BF).contains(&cp) || (0x1F300..=0x1FAFF).contains(&cp) || (0x1F600..=0x1F64F).contains(&cp)
}

pub fn run() -> i32 {
    let root = repo_root();
    let mut errors = Vec::new();
    let tracked = git_lines(&["ls-files"], &root);
    for rel in tracked {
        let ext = Path::new(&rel).extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_dot = format!(".{ext}");
        if !SCAN_EXT.contains(&ext_dot.as_str()) {
            continue;
        }
        let path = root.join(&rel);
        if !path.is_file() {
            continue;
        }
        let rel_norm = normalize_rel(&rel);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (i, line) in content.lines().enumerate() {
            scan_line(line, &rel_norm, i + 1, &mut errors);
        }
    }
    for e in &errors {
        eprintln!("error: {e}");
    }
    if errors.is_empty() {
        eprintln!("check text: ok");
        0
    } else {
        eprintln!("check text: {} error(s)", errors.len());
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_dashes_and_emoji() {
        let mut errors = Vec::new();
        scan_line("hello \u{2014} world", "a.md", 1, &mut errors);
        assert_eq!(errors.len(), 1);
        scan_line("a\u{2013}b", "b.md", 2, &mut errors);
        assert_eq!(errors.len(), 2);
        scan_line("ok", "c.md", 3, &mut errors);
        assert_eq!(errors.len(), 2);
    }
}
