use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::repo_root;

const MOD_MARKER: &str = "// scaffold:mod";
const REG_MARKER: &str = "// scaffold:register";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldPlan {
    pub feature_dir: PathBuf,
    pub files: Vec<String>,
    pub mod_line: String,
    pub register_line: String,
}

pub fn run(id: &str) -> i32 {
    match scaffold_feature(&repo_root(), id, false) {
        Ok(()) => {
            eprintln!("new-feature: created {id}");
            0
        }
        Err(e) => {
            eprintln!("new-feature: {e}");
            1
        }
    }
}

pub fn scaffold_feature(root: &Path, id: &str, dry_run: bool) -> Result<(), String> {
    validate_id(id)?;
    let plan = plan_scaffold(root, id)?;
    if dry_run {
        return Ok(());
    }
    if plan.feature_dir.exists() {
        return Err(format!("feature folder already exists: {}", plan.feature_dir.display()));
    }
    fs::create_dir_all(&plan.feature_dir).map_err(|e| e.to_string())?;
    for name in &plan.files {
        let src = templates_dir().join(name);
        let text = render_template(&fs::read_to_string(&src).map_err(|e| e.to_string())?, id)?;
        fs::write(plan.feature_dir.join(name), text).map_err(|e| e.to_string())?;
    }
    let mod_rs = features_mod_path(root);
    let before = fs::read_to_string(&mod_rs).map_err(|e| e.to_string())?;
    let after = patch_mod_rs(&before, id, &type_name(id))?;
    fs::write(&mod_rs, after).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn plan_scaffold(root: &Path, id: &str) -> Result<ScaffoldPlan, String> {
    validate_id(id)?;
    let feature_dir = features_root(root).join(id);
    let files = vec!["README.md".to_string(), "feature.toml".to_string(), "mod.rs".to_string(), "tests.rs".to_string()];
    Ok(ScaffoldPlan {
        feature_dir,
        files,
        mod_line: format!("pub mod {id};"),
        register_line: format!("    registry.register(Box::new({id}::{}::default()));", type_name(id)),
    })
}

pub fn patch_mod_rs(content: &str, id: &str, type_name: &str) -> Result<String, String> {
    if !content.contains(MOD_MARKER) || !content.contains(REG_MARKER) {
        return Err(format!("features/mod.rs missing {MOD_MARKER} or {REG_MARKER}"));
    }
    let mod_line = format!("pub mod {id};");
    let reg_line = format!("    registry.register(Box::new({id}::{type_name}::default()));");
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    if !content.contains(&mod_line) {
        let idx = lines.iter().position(|l| l.trim() == MOD_MARKER).ok_or("missing mod marker")?;
        let mut insert_at = idx + 1;
        while insert_at < lines.len() && lines[insert_at].trim().starts_with("pub mod ") {
            insert_at += 1;
        }
        lines.insert(insert_at, mod_line);
    }
    let joined = lines.join("\n");
    let mut lines: Vec<String> = joined.lines().map(String::from).collect();
    if !joined.contains(&reg_line) {
        let idx = lines.iter().position(|l| l.trim() == REG_MARKER).ok_or("missing register marker")?;
        let mut insert_at = idx + 1;
        while insert_at < lines.len() && lines[insert_at].trim().starts_with("registry.register") {
            insert_at += 1;
        }
        lines.insert(insert_at, reg_line);
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("feature id must not be empty".to_string());
    }
    let mut chars = id.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return Err(format!("feature id must start with a lowercase letter: {id}"));
    }
    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(format!("feature id must be snake_case: {id}"));
    }
    Ok(())
}

pub fn type_name(id: &str) -> String {
    let base: String = id
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
            }
        })
        .collect();
    format!("{base}Feature")
}

fn title_from_id(id: &str) -> String {
    id.split('_')
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_template(raw: &str, id: &str) -> Result<String, String> {
    let title = title_from_id(id);
    let type_name = type_name(id);
    Ok(raw.replace("{{id}}", id).replace("{{title}}", &title).replace("{{type_name}}", &type_name))
}

fn templates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates/feature")
}

fn features_root(root: &Path) -> PathBuf {
    root.join("kuro-rs/crates/kuro-features/src/features")
}

fn features_mod_path(root: &Path) -> PathBuf {
    features_root(root).join("mod.rs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dry_run_plan_paths() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_scaffold(dir.path(), "sample_demo").unwrap();
        assert!(
            plan.feature_dir.to_string_lossy().replace('\\', "/").ends_with("kuro-features/src/features/sample_demo")
        );
        assert_eq!(plan.files.len(), 4);
        assert_eq!(plan.mod_line, "pub mod sample_demo;");
        assert!(plan.register_line.contains("SampleDemoFeature"));
    }

    #[test]
    fn patch_inserts_registry_lines() {
        let before = r"//! reg

// scaffold:mod
// scaffold:register
";
        let after = patch_mod_rs(before, "sample_demo", "SampleDemoFeature").unwrap();
        assert!(after.contains("pub mod sample_demo;"));
        assert!(after.contains("registry.register(Box::new(sample_demo::SampleDemoFeature::default()));"));
    }

    #[test]
    fn patch_is_idempotent() {
        let before = r"// scaffold:mod
pub mod sample_demo;
pub fn register_all(registry: &mut FeatureRegistry) {
    // scaffold:register
    registry.register(Box::new(sample_demo::SampleDemoFeature::default()));
}
";
        let after = patch_mod_rs(before, "sample_demo", "SampleDemoFeature").unwrap();
        assert_eq!(after.matches("pub mod sample_demo;").count(), 1);
        assert_eq!(after.matches("registry.register(Box::new(sample_demo::SampleDemoFeature::default()));").count(), 1);
    }

    #[test]
    fn scaffold_writes_expected_files_in_temp() {
        let dir = tempfile::tempdir().unwrap();
        let features = dir.path().join("kuro-rs/crates/kuro-features/src/features");
        fs::create_dir_all(&features).unwrap();
        fs::write(
            features.join("mod.rs"),
            "//! reg\n\n// scaffold:mod\n\npub fn register_all(registry: &mut FeatureRegistry) {\n    // scaffold:register\n}\n",
        )
        .unwrap();
        scaffold_feature(dir.path(), "demo_feat", false).unwrap();
        assert!(features.join("demo_feat/feature.toml").is_file());
        let mod_rs = fs::read_to_string(features.join("mod.rs")).unwrap();
        assert!(mod_rs.contains("pub mod demo_feat;"));
        assert!(mod_rs.contains("DemoFeatFeature"));
    }
}
