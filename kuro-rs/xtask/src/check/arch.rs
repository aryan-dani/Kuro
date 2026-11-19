use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::paths::{data_dir, repo_root};

const FORBIDDEN_DEPS: &[&str] = &["windows", "windows-sys", "ort", "cpal", "rodio", "whisper-rs"];

pub fn run() -> i32 {
    let root = repo_root();
    let kuro_rs = root.join("kuro-rs");
    let metadata = match load_metadata(&kuro_rs) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("check arch: {e}");
            return 1;
        }
    };
    let portable = load_portable_crates();
    let mut errors: Vec<String> = Vec::new();
    for pkg in metadata_packages(&metadata) {
        let name = pkg_name(pkg);
        if !portable.contains(name) {
            continue;
        }
        for dep in forbidden_hits(pkg) {
            errors.push(format!("{name} must not depend on {dep} (ADR 0002 rule 2)"));
        }
    }
    for e in &errors {
        eprintln!("error: {e}");
    }
    if errors.is_empty() {
        eprintln!("check arch: ok ({} portable crate(s))", portable.len());
        0
    } else {
        eprintln!("check arch: {} error(s)", errors.len());
        1
    }
}

fn load_metadata(kuro_rs: &Path) -> Result<Value, String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version=1"])
        .current_dir(kuro_rs)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

fn load_portable_crates() -> HashSet<String> {
    let path = data_dir().join("portable_crates.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect()
}

fn metadata_packages(metadata: &Value) -> Vec<&Value> {
    metadata.get("packages").and_then(|p| p.as_array()).map(|arr| arr.iter().collect()).unwrap_or_default()
}

fn pkg_name(pkg: &Value) -> &str {
    pkg.get("name").and_then(|n| n.as_str()).unwrap_or("")
}

fn forbidden_hits(pkg: &Value) -> Vec<String> {
    let mut hits = Vec::new();
    let Some(deps) = pkg.get("dependencies").and_then(|d| d.as_array()) else {
        return hits;
    };
    for dep in deps {
        let Some(name) = dep.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        if FORBIDDEN_DEPS.contains(&name) {
            hits.push(name.to_string());
        }
    }
    hits.sort();
    hits.dedup();
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn forbidden_hits_detects_ort() {
        let pkg = json!({
            "name": "kuro-core",
            "dependencies": [
                { "name": "serde" },
                { "name": "ort" }
            ]
        });
        let hits = forbidden_hits(&pkg);
        assert_eq!(hits, vec!["ort".to_string()]);
    }

    #[test]
    fn synthetic_portable_crate_fails_rule() {
        let metadata = json!({
            "packages": [{
                "name": "kuro-core",
                "dependencies": [{ "name": "ort" }]
            }]
        });
        let mut errors = Vec::new();
        let portable: HashSet<String> = ["kuro-core".to_string()].into_iter().collect();
        for pkg in metadata_packages(&metadata) {
            let name = pkg_name(pkg);
            if portable.contains(name) {
                for dep in forbidden_hits(pkg) {
                    errors.push(format!("{name} must not depend on {dep}"));
                }
            }
        }
        assert_eq!(errors.len(), 1);
    }
}
