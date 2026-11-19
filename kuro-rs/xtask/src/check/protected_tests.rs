use super::*;
use crate::check::protected_adr::commit_has_adr_trailer;
use crate::paths::git_lines;
use std::fs;
use std::process::Command;

#[test]
fn glob_rules() {
    assert!(glob_matches("kuro-rs/crates/kuro-hotkey/**", "kuro-rs/crates/kuro-hotkey/src/lib.rs"));
    assert!(!glob_matches("kuro-rs/crates/kuro-hotkey/**", "kuro-rs/crates/kuro-router/src/lib.rs"));
    assert!(glob_matches("kuro-rs/crates/kuro-core/src/epoch.rs", "kuro-rs/crates/kuro-core/src/epoch.rs"));
}

#[test]
fn protected_without_adr_fails_in_temp_repo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    Command::new("git").args(["init"]).current_dir(root).output().unwrap();
    Command::new("git").args(["config", "user.email", "t@example.com"]).current_dir(root).output().unwrap();
    Command::new("git").args(["config", "user.name", "test"]).current_dir(root).output().unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(root.join("scripts/Test-KuroGate.ps1"), "# gate\n").unwrap();
    Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
    Command::new("git").args(["commit", "-m", "touch gate"]).current_dir(root).output().unwrap();
    let sha = git_lines(&["rev-parse", "HEAD"], root)[0].clone();
    assert!(commit_has_adr_trailer(&sha, root).is_none());
}
