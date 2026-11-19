use std::path::Path;

use crate::paths::git_lines;

pub fn commit_has_adr_trailer(sha: &str, root: &Path) -> Option<String> {
    let lines = git_lines(&["log", "-1", "--format=%B", sha], root);
    adr_from_body(&lines.join("\n"))
}

pub fn adr_file_exists(num: &str, root: &Path) -> bool {
    let adr_dir = root.join("docs/adr");
    if !adr_dir.is_dir() {
        return false;
    }
    std::fs::read_dir(adr_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().starts_with(&format!("{num}-")))
}

pub fn pending_commit_has_adr(root: &Path) -> bool {
    pending_commit_adr_number(root).is_some()
}

pub fn pending_commit_adr_number(root: &Path) -> Option<String> {
    if let Ok(msg) = std::env::var("KURO_COMMIT_MSG")
        && let Some(n) = adr_from_body(&msg)
    {
        return Some(n);
    }
    let edit = root.join(".git/COMMIT_EDITMSG");
    if edit.is_file()
        && let Ok(body) = std::fs::read_to_string(edit)
        && let Some(n) = adr_from_body(&body)
    {
        return Some(n);
    }
    None
}

pub fn adr_from_body(body: &str) -> Option<String> {
    for line in body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("ADR:") {
            let num = rest.split_whitespace().next().unwrap_or("");
            if !num.is_empty() {
                return Some(num.to_string());
            }
        }
    }
    None
}
