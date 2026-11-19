use std::path::Path;

use crate::paths::{git_lines, normalize_rel, repo_root};

use super::protected_adr::{
    adr_file_exists, commit_has_adr_trailer, pending_commit_adr_number, pending_commit_has_adr,
};

pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = normalize_rel(pattern);
    let path = normalize_rel(path);
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("/**").collect();
        if parts.len() == 2 {
            let prefix = parts[0].trim_end_matches('/');
            let suffix = parts[1].trim_start_matches('/');
            if suffix.is_empty() {
                return path == prefix || path.starts_with(&format!("{prefix}/"));
            }
            if path.starts_with(&format!("{prefix}/")) {
                let rest = &path[prefix.len() + 1..];
                return glob::Pattern::new(suffix).map(|p| p.matches(rest)).unwrap_or(false);
            }
            return false;
        }
    }
    glob::Pattern::new(&pattern).map(|p| p.matches(&path)).unwrap_or(false)
}

pub fn load_patterns() -> Vec<String> {
    let path = crate::paths::data_dir().join("protected_paths.txt");
    std::fs::read_to_string(path)
        .expect("protected_paths.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn changed_files(base: &str, root: &Path) -> Vec<String> {
    let mut files = git_lines(&["diff", "--name-only", &format!("{base}...HEAD")], root);
    if files.is_empty() {
        files = git_lines(&["diff", "--name-only", "--cached"], root);
    }
    files.into_iter().map(|f| normalize_rel(&f)).collect()
}

fn commits_touching(path: &str, base: &str, root: &Path) -> Vec<String> {
    git_lines(&["log", &format!("{base}..HEAD"), "--format=%H", "--", path], root)
}

pub fn run_staged() -> i32 {
    let root = repo_root();
    let patterns = load_patterns();
    let staged: Vec<String> =
        git_lines(&["diff", "--cached", "--name-only"], &root).into_iter().map(|f| normalize_rel(&f)).collect();
    let mut errors = Vec::new();
    for file in staged {
        if !patterns.iter().any(|p| glob_matches(p, &file)) {
            continue;
        }
        if !pending_commit_has_adr(&root) {
            errors.push(format!("{file}: staged protected edit; commit message must include an ADR: NNNN trailer"));
        } else if let Some(num) = pending_commit_adr_number(&root)
            && !adr_file_exists(&num, &root)
        {
            errors.push(format!("{file}: docs/adr/{num}-*.md missing"));
        }
    }
    report(errors, "check protected (staged)")
}

pub fn run(base: &str) -> i32 {
    let root = repo_root();
    let patterns = load_patterns();
    let changed = changed_files(base, &root);
    let mut errors = Vec::new();

    for file in changed {
        if !patterns.iter().any(|p| glob_matches(p, &file)) {
            continue;
        }
        let shas = commits_touching(&file, base, &root);
        if shas.is_empty() {
            let staged = git_lines(&["diff", "--cached", "--name-only", "--", &file], &root);
            if staged.is_empty() {
                errors.push(format!("{file}: protected change with no commits in range"));
                continue;
            }
        }
        let commits = if shas.is_empty() {
            vec![git_lines(&["rev-parse", "HEAD"], &root).first().cloned().unwrap_or_default()]
        } else {
            shas
        };
        for sha in commits {
            if sha.is_empty() {
                errors.push(format!("{file}: no commit to inspect for ADR trailer"));
                continue;
            }
            match commit_has_adr_trailer(&sha, &root) {
                None => errors.push(format!("{file}: commit {sha} missing ADR: trailer")),
                Some(num) if !adr_file_exists(&num, &root) => {
                    errors.push(format!("{file}: ADR {num} doc missing under docs/adr/"));
                }
                Some(_) => {}
            }
        }
    }

    if errors.is_empty() {
        eprintln!("check protected: ok (base {base})");
        0
    } else {
        report(errors, "check protected")
    }
}

fn report(errors: Vec<String>, label: &str) -> i32 {
    for e in &errors {
        eprintln!("error: {e}");
    }
    if errors.is_empty() {
        eprintln!("{label}: ok");
        0
    } else {
        eprintln!("{label}: {} error(s)", errors.len());
        1
    }
}

#[path = "protected_tests.rs"]
#[cfg(test)]
mod tests;
