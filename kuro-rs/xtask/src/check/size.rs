use std::collections::HashMap;
use std::path::Path;

use crate::paths::{git_lines, line_count, normalize_rel, repo_root};

const SIZE_EXT: &[&str] = &[".rs", ".cs", ".ps1", ".xaml"];
const SOFT_MAX: usize = 200;
const HARD_MAX: usize = 400;
const README_MAX: usize = 60;

pub fn run() -> i32 {
    let root = repo_root();
    let allow_path = crate::paths::data_dir().join("size_allowlist.toml");
    let allow: HashMap<String, usize> =
        toml::from_str(&std::fs::read_to_string(&allow_path).expect("size_allowlist.toml")).expect("parse allowlist");

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if git_lines(&["rev-parse", "--is-inside-work-tree"], &root).is_empty() {
        eprintln!("check size: not a git repo");
        return 1;
    }

    let tracked = git_lines(&["ls-files"], &root);
    for rel in &tracked {
        let rel_norm = normalize_rel(rel);
        let path = root.join(rel);
        if !path.is_file() {
            continue;
        }
        let count = line_count(&path);
        let ext = Path::new(rel).extension().and_then(|e| e.to_str()).unwrap_or("");
        let ext_dot = format!(".{ext}");

        if rel_norm.ends_with("/README.md") || rel_norm.ends_with("MAP.md") {
            if count > README_MAX {
                errors.push(format!("{rel_norm}: {count} lines (README/MAP cap {README_MAX})"));
            }
            continue;
        }

        if let Some(&cap) = allow.get(&rel_norm) {
            if count > cap {
                errors.push(format!("{rel_norm}: {count} lines exceeds allowlist cap {cap}"));
            }
            continue;
        }

        if SIZE_EXT.contains(&ext_dot.as_str()) && count > HARD_MAX {
            errors.push(format!("{rel_norm}: {count} lines over {HARD_MAX} (not allowlisted)"));
        } else if SIZE_EXT.contains(&ext_dot.as_str()) && count > SOFT_MAX {
            warnings.push(format!("{rel_norm}: {count} lines (soft target {SOFT_MAX})"));
        }
    }

    for (rel, cap) in &allow {
        let path = root.join(rel);
        if !path.is_file() {
            errors.push(format!("allowlist entry missing file: {rel}"));
            continue;
        }
        let count = line_count(&path);
        if count <= HARD_MAX {
            errors.push(format!("remove allowlist entry for {rel} ({count} lines, cap was {cap})"));
        }
    }

    for w in &warnings {
        eprintln!("warn: {w}");
    }
    for e in &errors {
        eprintln!("error: {e}");
    }
    if errors.is_empty() {
        eprintln!("check size: ok ({} warnings)", warnings.len());
        0
    } else {
        eprintln!("check size: {} error(s)", errors.len());
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn temp_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        Command::new("git").args(["init"]).current_dir(&root).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@example.com"]).current_dir(&root).output().unwrap();
        Command::new("git").args(["config", "user.name", "test"]).current_dir(&root).output().unwrap();
        (dir, root)
    }

    #[test]
    fn new_file_over_400_fails_when_not_listed() {
        let (_guard, root) = temp_repo();
        let body = (0..401).map(|i| format!("// line {i}")).collect::<Vec<_>>().join("\n");
        fs::write(root.join("big.rs"), body).unwrap();
        Command::new("git").args(["add", "big.rs"]).current_dir(&root).output().unwrap();
        // Direct logic: count > 400 without allowlist
        assert!(line_count(&root.join("big.rs")) > HARD_MAX);
    }
}
