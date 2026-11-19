use std::path::{Path, PathBuf};
use std::process::Command;

pub fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join(".git").is_dir() {
            return dir;
        }
        if !dir.pop() {
            panic!("no .git found above {}", env!("CARGO_MANIFEST_DIR"));
        }
    }
}

pub fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
}

pub fn git_lines(args: &[&str], cwd: &Path) -> Vec<String> {
    let out = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

pub fn normalize_rel(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn line_count(path: &Path) -> usize {
    std::fs::read_to_string(path).map(|s| s.lines().count()).unwrap_or(0)
}
