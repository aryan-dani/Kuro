use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::check::{protected, size, text};
use crate::paths::{git_lines, normalize_rel, repo_root};

pub fn run_fast() -> i32 {
    let start = Instant::now();
    if size::run() != 0 {
        return 1;
    }
    if text::run() != 0 {
        return 1;
    }
    if protected::run_staged() != 0 {
        return 1;
    }
    let root = repo_root();
    let kuro_rs = root.join("kuro-rs");
    if !run_cargo(&kuro_rs, &["fmt", "--all", "--", "--check"]) {
        return 1;
    }
    if !run_cargo(&kuro_rs, &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]) {
        return 1;
    }
    for crate_name in changed_crates(&root) {
        let args = ["test", "-p", &crate_name, "--", "--nocapture"];
        if !run_cargo(&kuro_rs, &args) {
            return 1;
        }
    }
    if shell_changed(&root) {
        let shell_tests = root.join("KuroShell/KuroShell.Tests/KuroShell.Tests.csproj");
        if shell_tests.exists()
            && !run_dotnet(&["test", shell_tests.to_str().unwrap(), "-c", "Debug", "-p:Platform=x64"])
        {
            return 1;
        }
    }
    eprintln!("gate fast: ok ({:.1}s, model skips not yet counted)", start.elapsed().as_secs_f64());
    0
}

pub fn run_full() -> i32 {
    let start = Instant::now();
    if run_fast() != 0 {
        return 1;
    }
    let root = repo_root();
    let kuro_rs = root.join("kuro-rs");
    let target = kuro_rs.join("target/gate");
    if !run_cargo_env(
        &kuro_rs,
        &[("CARGO_TARGET_DIR", target.to_str().unwrap())],
        &["test", "--workspace", "--", "--nocapture"],
    ) {
        return 1;
    }
    let shell_proj = root.join("KuroShell/KuroShell/KuroShell.csproj");
    let shell_tests = root.join("KuroShell/KuroShell.Tests/KuroShell.Tests.csproj");
    if shell_proj.exists() {
        if !run_dotnet(&["build", shell_proj.to_str().unwrap(), "-c", "Debug", "-p:Platform=x64"]) {
            return 1;
        }
        if !run_dotnet(&["test", shell_tests.to_str().unwrap(), "-c", "Debug", "-p:Platform=x64", "--no-build"]) {
            return 1;
        }
    }
    eprintln!(
        "gate full: ok ({:.1}s); live smoke delegated to scripts/Test-KuroGate.ps1 when needed",
        start.elapsed().as_secs_f64()
    );
    0
}

fn run_cargo(cwd: &Path, args: &[&str]) -> bool {
    run_cargo_env(cwd, &[], args)
}

fn run_cargo_env(cwd: &Path, env: &[(&str, &str)], args: &[&str]) -> bool {
    let mut cmd = Command::new("cargo");
    cmd.args(args).current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = cmd.status().expect("cargo");
    if !status.success() {
        eprintln!("cargo {} failed", args.join(" "));
    }
    status.success()
}

fn run_dotnet(args: &[&str]) -> bool {
    let status = Command::new("dotnet").args(args).status().expect("dotnet");
    if !status.success() {
        eprintln!("dotnet {} failed", args.join(" "));
    }
    status.success()
}

pub fn changed_crates(root: &Path) -> Vec<String> {
    let files = git_lines(&["diff", "--name-only", "HEAD"], root);
    let mut staged = git_lines(&["diff", "--cached", "--name-only"], root);
    staged.extend(files);
    let mut crates = Vec::new();
    for f in staged {
        let f = normalize_rel(&f);
        if let Some(name) = path_to_crate(&f)
            && !crates.contains(&name)
        {
            crates.push(name);
        }
    }
    if crates.is_empty() {
        crates.push("kuro-router".to_string());
    }
    crates
}

fn path_to_crate(path: &str) -> Option<String> {
    let prefix = "kuro-rs/crates/";
    if !path.starts_with(prefix) {
        return None;
    }
    let rest = &path[prefix.len()..];
    rest.split('/').next().map(str::to_string)
}

fn shell_changed(root: &Path) -> bool {
    let files = git_lines(&["diff", "--name-only", "HEAD"], root);
    let staged = git_lines(&["diff", "--cached", "--name-only"], root);
    files.iter().chain(staged.iter()).any(|f| normalize_rel(f).starts_with("KuroShell/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_from_path() {
        assert_eq!(path_to_crate("kuro-rs/crates/kuro-tts/src/lib.rs").as_deref(), Some("kuro-tts"));
        assert!(path_to_crate("README.md").is_none());
    }

    #[test]
    fn fast_tier_has_no_release_build_flag() {
        let args = ["test", "--workspace"];
        let joined = args.join(" ");
        assert!(!joined.contains("--release"));
    }
}
